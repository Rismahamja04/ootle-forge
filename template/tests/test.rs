// Engine tests for the Ootle Forge template.
//
// These run the real Ootle engine in-process via `tari_template_test_tooling`:
// the template is compiled to WASM and every transaction below executes
// against the same engine that runs on the network. No node, faucet or fees
// are involved. Run with `cargo test`.

use tari_template_lib::prelude::TARI_TOKEN;
use tari_template_lib::types::{
    Amount, ComponentAddress, NonFungibleAddress, NonFungibleId, ResourceAddress,
};
use tari_template_test_tooling::crypto::RistrettoSecretKey;
use tari_template_test_tooling::transaction::args;
use tari_template_test_tooling::TemplateTest;

const TEMPLATE_NAME: &str = "Forge";
/// Must match FORGE_FEE in src/lib.rs (1 TARI in microTari).
const FORGE_FEE: u64 = 1_000_000;
/// Must match MELT_VALUES in src/lib.rs.
const MELT_VALUES: [u64; 4] = [2, 6, 18, 54];

struct Setup {
    test: TemplateTest,
    forge: ComponentAddress,
    admin_account: ComponentAddress,
    admin_proof: NonFungibleAddress,
    admin_key: RistrettoSecretKey,
    badge_resource: ResourceAddress,
    ember_resource: ResourceAddress,
    relic_resource: ResourceAddress,
}

fn setup() -> Setup {
    let mut test = TemplateTest::my_crate();
    let (admin_account, admin_proof, admin_key) = test.create_funded_account();
    let template = test.get_template_address(TEMPLATE_NAME);

    let result = test.execute_expect_success(
        test.transaction()
            .call_function(template, "instantiate", args![])
            .put_last_instruction_output_on_workspace("badge")
            .call_method(admin_account, "deposit", args![Workspace("badge")])
            .build_and_seal(&admin_key),
        vec![admin_proof.clone()],
    );

    let forge = result
        .finalize
        .result
        .accept()
        .unwrap()
        .up_iter()
        .find(|(id, s)| {
            id.is_component()
                && *s.substate_value().component().unwrap().template_address() == template
        })
        .map(|(id, _)| id.as_component_address().unwrap())
        .unwrap();

    let ember_resource: ResourceAddress =
        test.call_method(forge, "ember_resource", args![], vec![]);
    let relic_resource: ResourceAddress =
        test.call_method(forge, "relic_resource", args![], vec![]);

    // The admin badge is the one NFT resource in the admin account that is
    // neither EMBER nor RELIC.
    let vaults = test
        .read_only_state_store()
        .get_vaults_for_account(admin_account)
        .unwrap();
    let badge_resource = *vaults
        .keys()
        .find(|r| **r != ember_resource && **r != relic_resource && **r != TARI_TOKEN)
        .expect("admin badge resource not found");

    Setup {
        test,
        forge,
        admin_account,
        admin_proof,
        admin_key,
        badge_resource,
        ember_resource,
        relic_resource,
    }
}

fn balance(test: &mut TemplateTest, account: ComponentAddress, resource: ResourceAddress) -> Amount {
    test.call_method(account, "balance", args![resource], vec![])
}

fn stats(test: &mut TemplateTest, forge: ComponentAddress) -> (u64, u64, u64, u64, Amount, u64) {
    test.call_method(forge, "stats", args![], vec![])
}

fn gather(
    test: &mut TemplateTest,
    forge: ComponentAddress,
    account: ComponentAddress,
    proof: &NonFungibleAddress,
    key: &RistrettoSecretKey,
) {
    test.execute_expect_success(
        test.transaction()
            .call_method(forge, "gather", args![])
            .put_last_instruction_output_on_workspace("embers")
            .call_method(account, "deposit", args![Workspace("embers")])
            .build_and_seal(key),
        vec![proof.clone()],
    );
}

/// Withdraw exactly 5 EMBER and the 1 TARI fee from the player's account,
/// forge a Relic and deposit it back. Returns the new Relic's serial.
fn forge_relic(
    test: &mut TemplateTest,
    forge: ComponentAddress,
    account: ComponentAddress,
    proof: &NonFungibleAddress,
    key: &RistrettoSecretKey,
    ember_resource: ResourceAddress,
) -> u64 {
    let serial = stats(test, forge).5;
    test.execute_expect_success(
        test.transaction()
            .call_method(account, "withdraw", args![ember_resource, Amount::from(5u64)])
            .put_last_instruction_output_on_workspace("embers")
            .call_method(account, "withdraw", args![TARI_TOKEN, Amount::from(FORGE_FEE)])
            .put_last_instruction_output_on_workspace("fee")
            .call_method(
                forge,
                "forge",
                args![Workspace("embers"), Workspace("fee")],
            )
            .put_last_instruction_output_on_workspace("relic")
            .call_method(account, "deposit", args![Workspace("relic")])
            .build_and_seal(key),
        vec![proof.clone()],
    );
    serial
}

fn relic_tier(test: &mut TemplateTest, forge: ComponentAddress, serial: u64) -> u8 {
    test.call_method(
        forge,
        "relic_tier",
        args![NonFungibleId::from_u64(serial)],
        vec![],
    )
}

#[test]
fn gather_mints_five_embers_per_call() {
    let Setup {
        mut test,
        forge,
        ember_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    gather(&mut test, forge, player, &player_proof, &player_key);
    assert_eq!(balance(&mut test, player, ember_resource), Amount::from(5u64));

    gather(&mut test, forge, player, &player_proof, &player_key);
    assert_eq!(balance(&mut test, player, ember_resource), Amount::from(10u64));

    let (gathered, ..) = stats(&mut test, forge);
    assert_eq!(gathered, 10);
}

#[test]
fn forge_mints_a_relic_and_banks_the_fee() {
    let Setup {
        mut test,
        forge,
        ember_resource,
        relic_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    let tari_before = balance(&mut test, player, TARI_TOKEN);
    gather(&mut test, forge, player, &player_proof, &player_key);
    let serial = forge_relic(
        &mut test,
        forge,
        player,
        &player_proof,
        &player_key,
        ember_resource,
    );

    assert_eq!(serial, 0);
    assert_eq!(balance(&mut test, player, relic_resource), Amount::from(1u64));
    assert_eq!(balance(&mut test, player, ember_resource), Amount::from(0u64));
    assert_eq!(
        balance(&mut test, player, TARI_TOKEN),
        tari_before - Amount::from(FORGE_FEE)
    );

    let (_, forged, _, _, fees, next_serial) = stats(&mut test, forge);
    assert_eq!(forged, 1);
    assert_eq!(fees, Amount::from(FORGE_FEE));
    assert_eq!(next_serial, 1);

    // The minted Relic carries a valid tier.
    let tier = relic_tier(&mut test, forge, serial);
    assert!(tier <= 3, "tier {tier} out of range");
}

#[test]
fn forge_rejects_the_wrong_ember_amount() {
    let Setup {
        mut test,
        forge,
        ember_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    gather(&mut test, forge, player, &player_proof, &player_key);

    // Offer 4 EMBER instead of 5: the whole transaction must be rejected and
    // nothing may change.
    test.execute_expect_failure(
        test.transaction()
            .call_method(player, "withdraw", args![ember_resource, Amount::from(4u64)])
            .put_last_instruction_output_on_workspace("embers")
            .call_method(player, "withdraw", args![TARI_TOKEN, Amount::from(FORGE_FEE)])
            .put_last_instruction_output_on_workspace("fee")
            .call_method(
                forge,
                "forge",
                args![Workspace("embers"), Workspace("fee")],
            )
            .build_and_seal(&player_key),
        vec![player_proof.clone()],
    );

    assert_eq!(balance(&mut test, player, ember_resource), Amount::from(5u64));
    let (_, forged, _, _, fees, _) = stats(&mut test, forge);
    assert_eq!(forged, 0);
    assert_eq!(fees, Amount::from(0u64));
}

#[test]
fn forge_rejects_a_short_fee() {
    let Setup {
        mut test,
        forge,
        ember_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    gather(&mut test, forge, player, &player_proof, &player_key);

    // 999_999 microTari is one microTari short of the 1 TARI fee.
    test.execute_expect_failure(
        test.transaction()
            .call_method(player, "withdraw", args![ember_resource, Amount::from(5u64)])
            .put_last_instruction_output_on_workspace("embers")
            .call_method(
                player,
                "withdraw",
                args![TARI_TOKEN, Amount::from(FORGE_FEE - 1)],
            )
            .put_last_instruction_output_on_workspace("fee")
            .call_method(
                forge,
                "forge",
                args![Workspace("embers"), Workspace("fee")],
            )
            .build_and_seal(&player_key),
        vec![player_proof.clone()],
    );

    let (_, forged, ..) = stats(&mut test, forge);
    assert_eq!(forged, 0);
}

#[test]
fn melt_burns_the_relic_and_pays_its_tier_value() {
    let Setup {
        mut test,
        forge,
        ember_resource,
        relic_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    gather(&mut test, forge, player, &player_proof, &player_key);
    let serial = forge_relic(
        &mut test,
        forge,
        player,
        &player_proof,
        &player_key,
        ember_resource,
    );
    let tier = relic_tier(&mut test, forge, serial);

    test.execute_expect_success(
        test.transaction()
            .call_method(
                player,
                "withdraw_non_fungible",
                args![relic_resource, NonFungibleId::from_u64(serial)],
            )
            .put_last_instruction_output_on_workspace("relic")
            .call_method(forge, "melt", args![Workspace("relic")])
            .put_last_instruction_output_on_workspace("embers")
            .call_method(player, "deposit", args![Workspace("embers")])
            .build_and_seal(&player_key),
        vec![player_proof.clone()],
    );

    assert_eq!(balance(&mut test, player, relic_resource), Amount::from(0u64));
    assert_eq!(
        balance(&mut test, player, ember_resource),
        Amount::from(MELT_VALUES[tier as usize])
    );
    let (_, _, _, melted, _, _) = stats(&mut test, forge);
    assert_eq!(melted, 1);
}

#[test]
fn ascend_combines_three_of_a_kind_into_the_next_tier() {
    let Setup {
        mut test,
        forge,
        ember_resource,
        relic_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    // Forge until the player holds three Relics of one tier. Commons are the
    // most likely outcome (62%), so this terminates quickly in practice; the
    // cap exists only to keep a pathological run from looping forever.
    let mut by_tier: [Vec<u64>; 4] = Default::default();
    let mut target: Option<(u8, Vec<u64>)> = None;
    for _ in 0..60 {
        gather(&mut test, forge, player, &player_proof, &player_key);
        let serial = forge_relic(
            &mut test,
            forge,
            player,
            &player_proof,
            &player_key,
            ember_resource,
        );
        let tier = relic_tier(&mut test, forge, serial);
        by_tier[tier as usize].push(serial);
        if by_tier[tier as usize].len() == 3 && tier < 3 {
            target = Some((tier, by_tier[tier as usize].clone()));
            break;
        }
    }
    let (tier, serials) = target.expect("no three-of-a-kind forged within 60 forges");
    let relics_before = balance(&mut test, player, relic_resource);

    let mut tx = test.transaction();
    for (i, serial) in serials.iter().enumerate() {
        tx = tx
            .call_method(
                player,
                "withdraw_non_fungible",
                args![relic_resource, NonFungibleId::from_u64(*serial)],
            )
            .put_last_instruction_output_on_workspace(format!("relic{i}"));
    }
    let ascended_serial = stats(&mut test, forge).5;
    test.execute_expect_success(
        tx.call_method(
            forge,
            "ascend",
            args![
                Workspace("relic0"),
                Workspace("relic1"),
                Workspace("relic2")
            ],
        )
        .put_last_instruction_output_on_workspace("ascended")
        .call_method(player, "deposit", args![Workspace("ascended")])
        .build_and_seal(&player_key),
        vec![player_proof.clone()],
    );

    // Three burned, one minted: the collection shrinks by two.
    assert_eq!(
        balance(&mut test, player, relic_resource),
        relics_before - Amount::from(2u64)
    );
    assert_eq!(
        relic_tier(&mut test, forge, ascended_serial),
        tier + 1,
        "ascended Relic must be exactly one tier up"
    );
    let (_, _, ascended, _, _, _) = stats(&mut test, forge);
    assert_eq!(ascended, 1);
}

#[test]
fn ascend_rejects_mixed_tiers() {
    let Setup {
        mut test,
        forge,
        ember_resource,
        relic_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    // Forge until the player holds Relics of at least two different tiers,
    // plus a third Relic of either of those tiers.
    let mut relics: Vec<(u64, u8)> = Vec::new();
    for _ in 0..60 {
        gather(&mut test, forge, player, &player_proof, &player_key);
        let serial = forge_relic(
            &mut test,
            forge,
            player,
            &player_proof,
            &player_key,
            ember_resource,
        );
        let tier = relic_tier(&mut test, forge, serial);
        relics.push((serial, tier));
        let distinct: Vec<u8> = relics.iter().map(|(_, t)| *t).collect();
        if relics.len() >= 3 && distinct.iter().any(|t| *t != distinct[0]) {
            break;
        }
    }
    assert!(relics.len() >= 3, "not enough relics forged");
    let first_tier = relics[0].1;
    let other = relics
        .iter()
        .find(|(_, t)| *t != first_tier)
        .expect("no mixed tier forged within 60 forges");
    let picks = [relics[0].0, other.0, relics[2].0];

    let mut tx = test.transaction();
    for (i, serial) in picks.iter().enumerate() {
        tx = tx
            .call_method(
                player,
                "withdraw_non_fungible",
                args![relic_resource, NonFungibleId::from_u64(*serial)],
            )
            .put_last_instruction_output_on_workspace(format!("relic{i}"));
    }
    test.execute_expect_failure(
        tx.call_method(
            forge,
            "ascend",
            args![
                Workspace("relic0"),
                Workspace("relic1"),
                Workspace("relic2")
            ],
        )
        .build_and_seal(&player_key),
        vec![player_proof.clone()],
    );

    let (_, _, ascended, _, _, _) = stats(&mut test, forge);
    assert_eq!(ascended, 0);
}

#[test]
fn withdraw_fees_requires_the_admin_badge_and_pays_out() {
    let Setup {
        mut test,
        forge,
        admin_account,
        admin_proof,
        admin_key,
        badge_resource,
        ember_resource,
        ..
    } = setup();
    let (player, player_proof, player_key) = test.create_funded_account();

    // Collect one forge fee first.
    gather(&mut test, forge, player, &player_proof, &player_key);
    forge_relic(
        &mut test,
        forge,
        player,
        &player_proof,
        &player_key,
        ember_resource,
    );
    let (_, _, _, _, fees, _) = stats(&mut test, forge);
    assert_eq!(fees, Amount::from(FORGE_FEE));

    // A player without the badge cannot withdraw the fees: they cannot even
    // create a proof of a badge they do not hold.
    test.execute_expect_failure(
        test.transaction()
            .create_proof(player, badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(forge, "withdraw_fees", args![])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&player_key),
        vec![player_proof.clone()],
    );

    // The badge holder can, and receives exactly the collected fees.
    let admin_tari_before = balance(&mut test, admin_account, TARI_TOKEN);
    test.execute_expect_success(
        test.transaction()
            .create_proof(admin_account, badge_resource)
            .put_last_instruction_output_on_workspace("proof")
            .authorize_proof("proof")
            .call_method(forge, "withdraw_fees", args![])
            .put_last_instruction_output_on_workspace("fees")
            .call_method(admin_account, "deposit", args![Workspace("fees")])
            .drop_all_proofs_in_workspace()
            .build_and_seal(&admin_key),
        vec![admin_proof.clone()],
    );

    assert_eq!(
        balance(&mut test, admin_account, TARI_TOKEN),
        admin_tari_before + Amount::from(FORGE_FEE)
    );
    let (_, _, _, _, fees_after, _) = stats(&mut test, forge);
    assert_eq!(fees_after, Amount::from(0u64));
}
