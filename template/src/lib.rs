// Ootle Forge — a collectible crafting game on Tari Ootle.
//
// Gather EMBER, burn it (plus a small TARI fee) to forge Relic NFTs with a
// rolled rarity tier, ascend three same-tier Relics into one of the next tier,
// and melt duplicates back into EMBER. All minting authority for both resources
// is locked to this component, so a Relic can only ever come into existence
// through `forge` or `ascend` — not even the operator can mint one directly.
//
// MIT License.

use tari_template_lib::prelude::*;

/// How many EMBER one `gather()` call mints.
pub const GATHER_AMOUNT: u64 = 5;
/// EMBER burned per `forge()` call.
pub const FORGE_EMBER_COST: u64 = 5;
/// Forge fee in microTari (1 TARI = 1_000_000 microTari).
pub const FORGE_FEE: u64 = 1_000_000;
/// EMBER returned by `melt()` per tier: Common, Rare, Epic, Legendary.
pub const MELT_VALUES: [u64; 4] = [2, 6, 18, 54];
/// Highest tier (Legendary). Legendaries cannot ascend.
pub const MAX_TIER: u8 = 3;

/// Immutable data carried by every Relic NFT.
#[derive(Debug, Clone, minicbor::Encode, minicbor::Decode, minicbor::CborLen)]
pub struct RelicData {
    /// 0 = Common, 1 = Rare, 2 = Epic, 3 = Legendary.
    #[n(0)]
    pub tier: u8,
    /// Sequential forge number; also the NFT's id (`NonFungibleId::from_u64`).
    #[n(1)]
    pub serial: u64,
}

/// Roll a tier from one entropy byte: 62% Common, 26% Rare, 9% Epic, 3% Legendary.
fn tier_from_roll(roll: u8) -> u8 {
    match roll % 100 {
        0..=61 => 0,
        62..=87 => 1,
        88..=96 => 2,
        _ => 3,
    }
}

#[template]
mod forge {
    use super::*;
    use tari_template_lib::rand::random_bytes;

    pub struct Forge {
        ember_resource: ResourceAddress,
        relic_resource: ResourceAddress,
        fee_vault: Vault,
        next_serial: u64,
        embers_gathered: u64,
        relics_forged: u64,
        relics_ascended: u64,
        relics_melted: u64,
    }

    impl Forge {
        /// Create a new Forge. Returns a bucket holding the single admin badge;
        /// whoever holds that badge may call `withdraw_fees`.
        pub fn instantiate() -> Bucket {
            let component_alloc = CallerContext::allocate_component_address(None);
            let component_address = component_alloc.get_address();

            // The admin badge: a single NFT (id 0) whose holder is the operator.
            let admin_badge = ResourceBuilder::non_fungible()
                .with_token_symbol("FORGEADMIN")
                .metadata("name", "Ootle Forge Admin Badge")
                .initial_supply(Some(NonFungibleId::from_u64(0)));
            let admin_badge_resource = admin_badge.resource_address();

            // EMBER: the game's fungible crafting material. Only this component
            // may mint it, and that rule is LOCKED at creation.
            let ember_resource = ResourceBuilder::public_fungible()
                .with_token_symbol("EMBER")
                .metadata("name", "Forge Ember")
                .mintable(rule!(component(component_address)), LOCKED)
                .burnable(rule!(allow_all), LOCKED)
                .build();

            // RELIC: the collectible NFT. Same locked, component-only minting.
            let relic_resource = ResourceBuilder::non_fungible()
                .with_token_symbol("RELIC")
                .metadata("name", "Forge Relic")
                .mintable(rule!(component(component_address)), LOCKED)
                .burnable(rule!(allow_all), LOCKED)
                .build();

            let access_rules = AccessRules::new()
                .add_method_rule("gather", AccessRule::AllowAll)
                .add_method_rule("forge", AccessRule::AllowAll)
                .add_method_rule("ascend", AccessRule::AllowAll)
                .add_method_rule("melt", AccessRule::AllowAll)
                .add_method_rule("stats", AccessRule::AllowAll)
                .add_method_rule("ember_resource", AccessRule::AllowAll)
                .add_method_rule("relic_resource", AccessRule::AllowAll)
                .add_method_rule("relic_tier", AccessRule::AllowAll)
                .add_method_rule(
                    "withdraw_fees",
                    rule!(resource(admin_badge_resource)),
                )
                .default(AccessRule::DenyAll);

            Component::new(Self {
                ember_resource,
                relic_resource,
                fee_vault: Vault::new_empty(TARI_TOKEN),
                next_serial: 0,
                embers_gathered: 0,
                relics_forged: 0,
                relics_ascended: 0,
                relics_melted: 0,
            })
            .with_address_allocation(component_alloc)
            .with_access_rules(access_rules)
            // No owner: the access rules above can never be changed, and the
            // component can never be upgraded (the default upgrade rule).
            .with_owner_rule(OwnerRule::None)
            .create();

            admin_badge
        }

        /// Mint 5 fresh EMBER to the caller. This is the game's faucet: on a
        /// live network it is how new players get crafting material.
        pub fn gather(&mut self) -> Bucket {
            self.embers_gathered += GATHER_AMOUNT;
            let embers = ResourceManager::get(self.ember_resource)
                .mint_fungible(Amount::from(GATHER_AMOUNT));
            emit_event("gather", [("amount", GATHER_AMOUNT.to_string())]);
            embers
        }

        /// Burn exactly 5 EMBER plus a fee of exactly 1 TARI and receive one
        /// Relic NFT of a rolled tier.
        pub fn forge(&mut self, embers: Bucket, fee: Bucket) -> Bucket {
            assert_eq!(
                embers.resource_address(),
                self.ember_resource,
                "forge requires an EMBER bucket"
            );
            assert_eq!(
                embers.amount(),
                Amount::from(FORGE_EMBER_COST),
                "forging a Relic costs exactly {} EMBER",
                FORGE_EMBER_COST
            );
            assert_eq!(
                fee.resource_address(),
                TARI_TOKEN,
                "the forge fee must be paid in TARI"
            );
            assert_eq!(
                fee.amount(),
                Amount::from(FORGE_FEE),
                "the forge fee is exactly 1 TARI"
            );

            embers.burn();
            self.fee_vault.deposit(fee);

            let tier = tier_from_roll(random_bytes(1)[0]);
            let relic = self.mint_relic(tier);
            self.relics_forged += 1;
            relic
        }

        /// Burn three Relics of the same tier and receive one Relic of the
        /// next tier. Legendaries (tier 3) cannot ascend.
        pub fn ascend(&mut self, first: Bucket, second: Bucket, third: Bucket) -> Bucket {
            let tier_a = self.relic_tier_of_bucket(&first);
            let tier_b = self.relic_tier_of_bucket(&second);
            let tier_c = self.relic_tier_of_bucket(&third);
            assert!(
                tier_a == tier_b && tier_b == tier_c,
                "ascend requires three Relics of the same tier"
            );
            assert!(
                tier_a < MAX_TIER,
                "Legendary Relics cannot ascend further"
            );

            first.burn();
            second.burn();
            third.burn();

            let relic = self.mint_relic(tier_a + 1);
            self.relics_ascended += 1;
            relic
        }

        /// Burn one Relic and receive its tier's melt value in EMBER, so
        /// duplicates are never dead weight.
        pub fn melt(&mut self, relic: Bucket) -> Bucket {
            let tier = self.relic_tier_of_bucket(&relic);
            relic.burn();
            self.relics_melted += 1;
            let embers = ResourceManager::get(self.ember_resource)
                .mint_fungible(Amount::from(MELT_VALUES[tier as usize]));
            emit_event(
                "melt",
                [
                    ("tier", tier.to_string()),
                    ("embers", MELT_VALUES[tier as usize].to_string()),
                ],
            );
            embers
        }

        /// Lifetime counters and the fees collected so far:
        /// (embers gathered, relics forged, relics ascended, relics melted,
        ///  fee balance in microTari, next relic serial).
        pub fn stats(&self) -> (u64, u64, u64, u64, Amount, u64) {
            (
                self.embers_gathered,
                self.relics_forged,
                self.relics_ascended,
                self.relics_melted,
                self.fee_vault.balance(),
                self.next_serial,
            )
        }

        pub fn ember_resource(&self) -> ResourceAddress {
            self.ember_resource
        }

        pub fn relic_resource(&self) -> ResourceAddress {
            self.relic_resource
        }

        /// Read the tier of any Relic by id (used by indexers and the demo).
        pub fn relic_tier(&self, id: NonFungibleId) -> u8 {
            ResourceManager::get(self.relic_resource)
                .get_non_fungible(&id)
                .get_data::<RelicData>()
                .tier
        }

        /// Withdraw all collected forge fees. Admin badge holders only
        /// (enforced by the component access rules).
        pub fn withdraw_fees(&mut self) -> Bucket {
            let fees = self.fee_vault.withdraw_all();
            emit_event("withdraw_fees", [("amount", fees.amount().to_string())]);
            fees
        }

        /// Mint the next Relic of `tier` and return it as a bucket.
        fn mint_relic(&mut self, tier: u8) -> Bucket {
            let serial = self.next_serial;
            self.next_serial += 1;
            let relic = ResourceManager::get(self.relic_resource).mint_non_fungible(
                NonFungibleId::from_u64(serial),
                &RelicData { tier, serial },
                &(),
            );
            emit_event(
                "relic_minted",
                [("serial", serial.to_string()), ("tier", tier.to_string())],
            );
            relic
        }

        /// Assert a bucket holds exactly one Relic of this Forge and return
        /// its tier.
        fn relic_tier_of_bucket(&self, bucket: &Bucket) -> u8 {
            assert_eq!(
                bucket.resource_address(),
                self.relic_resource,
                "expected a Relic bucket"
            );
            let ids = bucket.get_non_fungible_ids();
            assert_eq!(ids.len(), 1, "expected exactly one Relic");
            let id = ids.iter().next().expect("one id checked above");
            ResourceManager::get(self.relic_resource)
                .get_non_fungible(id)
                .get_data::<RelicData>()
                .tier
        }
    }
}
