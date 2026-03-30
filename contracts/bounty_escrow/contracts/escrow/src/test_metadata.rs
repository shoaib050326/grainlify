use crate::{validation::MAX_TAG_LEN, BountyEscrowContract, BountyEscrowContractClient};
use soroban_sdk::{testutils::Address as _, Address, Bytes, Env, String};

fn setup() -> (Env, Address, BountyEscrowContractClient<'static>) {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register_contract(None, BountyEscrowContract);
    let client = BountyEscrowContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let token = Address::generate(&env);
    client.init(&admin, &token);

    (env, admin, client)
}

#[test]
fn test_metadata_storage_and_query() {
    let (env, admin, client) = setup();

    let bounty_id = 1u64;
    let repo_id = 12345u64;
    let issue_id = 67890u64;
    let b_type = String::from_str(&env, "bounty");

    // 2. Set Metadata (requires admin auth)
    client.update_metadata(&admin, &bounty_id, &repo_id, &issue_id, &b_type, &None);

    // 3. Verify retrieval
    let fetched = client.get_metadata(&bounty_id);
    assert_eq!(fetched.repo_id, repo_id);
    assert_eq!(fetched.issue_id, issue_id);
    assert_eq!(fetched.bounty_type, b_type);
}

#[test]
fn test_metadata_accepts_len_one_boundary() {
    let (env, admin, client) = setup();
    let bounty_type = String::from_str(&env, "a");

    client.update_metadata(&admin, &2u64, &10u64, &20u64, &bounty_type, &None);

    assert_eq!(client.get_metadata(&2u64).bounty_type, bounty_type);
}

#[test]
fn test_metadata_accepts_len_max_boundary() {
    let (env, admin, client) = setup();
    let bounty_type = String::from_str(&env, &"a".repeat(MAX_TAG_LEN));
    let reference_hash = Some(Bytes::from_slice(&env, &[1, 2, 3, 4]));

    client.update_metadata(&admin, &3u64, &11u64, &21u64, &bounty_type, &reference_hash);

    let fetched = client.get_metadata(&3u64);
    assert_eq!(fetched.bounty_type, bounty_type);
    assert_eq!(fetched.reference_hash, reference_hash);
}

#[test]
#[should_panic(expected = "bounty_type cannot be empty")]
fn test_metadata_rejects_empty_bounty_type() {
    let (env, admin, client) = setup();
    let empty = String::from_str(&env, "");

    client.update_metadata(&admin, &4u64, &12u64, &22u64, &empty, &None);
}

#[test]
#[should_panic(expected = "bounty_type exceeds maximum length of 50 characters")]
fn test_metadata_rejects_bounty_type_above_max_len() {
    let (env, admin, client) = setup();
    let too_long = String::from_str(&env, &"a".repeat(MAX_TAG_LEN + 1));

    client.update_metadata(&admin, &5u64, &13u64, &23u64, &too_long, &None);
}

#[test]
fn test_metadata_accepts_sdk_permitted_unicode_edge_cases() {
    let (env, admin, client) = setup();
    let cases = [
        "naive",
        "na\u{00ef}ve",
        "cafe\u{301}",
        "\u{4f60}\u{597d}",
        "\u{1f980}",
        "bug-fix/v2",
    ];

    for (idx, case) in cases.iter().enumerate() {
        let bounty_id = 100u64 + idx as u64;
        let bounty_type = String::from_str(&env, case);

        client.update_metadata(
            &admin,
            &bounty_id,
            &(500u64 + idx as u64),
            &(900u64 + idx as u64),
            &bounty_type,
            &None,
        );

        assert_eq!(client.get_metadata(&bounty_id).bounty_type, bounty_type);
    }
}
