use super::*;
#[test]
fn argon2id_hashes_are_salted_and_verifiable() {
    let policy = Argon2idPolicy::new(19_456, 2, 1).unwrap();
    let first = hash_password("correct horse battery staple", policy).unwrap();
    let second = hash_password("correct horse battery staple", policy).unwrap();
    assert_ne!(first, second);
    assert!(first.starts_with("$argon2id$"));
    verify_password("correct horse battery staple", &first, policy).unwrap();
    assert!(matches!(
        verify_password("incorrect password", &first, policy),
        Err(AuthError::InvalidCredentials)
    ));
}
#[test]
fn argon2id_policy_rejects_zero_costs_and_is_embedded_in_hash() {
    assert!(matches!(
        Argon2idPolicy::new(0, 2, 1),
        Err(AuthError::InvalidHashPolicy)
    ));
    let policy = Argon2idPolicy::new(8192, 3, 2).unwrap();
    let encoded = hash_password("correct horse battery staple", policy).unwrap();
    assert!(encoded.contains("m=8192,t=3,p=2"));
}
#[test]
fn opaque_tokens_are_stored_only_as_fixed_length_verifiers() {
    let raw = random_token();
    let verifier = token_hash(&raw);
    assert_eq!(raw.len(), 64);
    assert_eq!(verifier.len(), 64);
    assert_ne!(raw, verifier);
    assert_eq!(verifier, token_hash(&raw));
}
#[test]
fn password_and_username_boundaries_reject_ambiguous_input() {
    assert!(matches!(
        validate_password("too short"),
        Err(AuthError::WeakPassword)
    ));
    assert!(matches!(
        validate_username(" alice"),
        Err(AuthError::InvalidUsername)
    ));
    assert!(validate_username("alice@example.test").is_ok());
}
