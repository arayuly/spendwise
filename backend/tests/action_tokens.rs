use backend::{
    auth_tokens::{self, TokenPurpose},
    error::AppError,
};
use sqlx::PgPool;
use uuid::Uuid;

#[sqlx::test(migrations = "./migrations")]
async fn tokens_are_hashed_one_time_scoped_and_expiring(pool: PgPool) {
    let user = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,password_hash,display_name) VALUES($1,'tokens@example.com','unused-test-hash','Token test')").bind(user).execute(&pool).await.unwrap();
    let first = auth_tokens::issue(&pool, user, TokenPurpose::ResetPassword)
        .await
        .unwrap();
    let second = auth_tokens::issue(&pool, user, TokenPurpose::ResetPassword)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        auth_tokens::consume(&mut tx, &first, TokenPurpose::ResetPassword).await,
        Err(AppError::Unauthorized)
    ));
    assert!(matches!(
        auth_tokens::consume(&mut tx, &second, TokenPurpose::VerifyEmail).await,
        Err(AppError::Unauthorized)
    ));
    assert_eq!(
        auth_tokens::consume(&mut tx, &second, TokenPurpose::ResetPassword)
            .await
            .unwrap(),
        user
    );
    assert!(matches!(
        auth_tokens::consume(&mut tx, &second, TokenPurpose::ResetPassword).await,
        Err(AppError::Unauthorized)
    ));
    tx.commit().await.unwrap();
    let third = auth_tokens::issue(&pool, user, TokenPurpose::VerifyEmail)
        .await
        .unwrap();
    sqlx::query("UPDATE auth_action_tokens SET created_at=now()-interval '2 days', expires_at=now()-interval '1 day' WHERE user_id=$1 AND purpose='verify_email'").bind(user).execute(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        auth_tokens::consume(&mut tx, &third, TokenPurpose::VerifyEmail).await,
        Err(AppError::Unauthorized)
    ));
    let stored: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM auth_action_tokens")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(
        stored
            .iter()
            .all(|hash| hash != &first && hash != &second && hash != &third)
    );
}

async fn insert_action_user(pool: &PgPool) -> Uuid {
    let user = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,password_hash,display_name) VALUES($1,$2,'test-only-hash','Concurrent token test')")
        .bind(user)
        .bind(format!("{user}@example.test"))
        .execute(pool)
        .await
        .unwrap();
    user
}

async fn consume_password_token(pool: &PgPool, raw: &str) -> Result<Uuid, AppError> {
    let mut tx = pool.begin().await?;
    let user = auth_tokens::consume(&mut tx, raw, TokenPurpose::ResetPassword).await?;
    tx.commit().await?;
    Ok(user)
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_double_consumption_succeeds_exactly_once(pool: PgPool) {
    let user = insert_action_user(&pool).await;
    let raw = auth_tokens::issue(&pool, user, TokenPurpose::ResetPassword)
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        consume_password_token(&pool, &raw),
        consume_password_token(&pool, &raw)
    );
    match (first, second) {
        (Ok(found), Err(AppError::Unauthorized)) | (Err(AppError::Unauthorized), Ok(found)) => {
            assert_eq!(found, user);
        }
        other => panic!("expected one successful consumption, got {other:?}"),
    }
    assert!(matches!(
        consume_password_token(&pool, &raw).await,
        Err(AppError::Unauthorized)
    ));
}

#[sqlx::test(migrations = "./migrations")]
async fn reissue_and_consume_share_account_before_token_lock_order(pool: PgPool) {
    let user = insert_action_user(&pool).await;
    let raw = auth_tokens::issue(&pool, user, TokenPurpose::ResetPassword)
        .await
        .unwrap();
    let mut consume = pool.begin().await.unwrap();
    assert_eq!(
        auth_tokens::consume(&mut consume, &raw, TokenPurpose::ResetPassword)
            .await
            .unwrap(),
        user
    );

    let issue_pool = pool.clone();
    let mut reissue = tokio::spawn(async move {
        auth_tokens::issue(&issue_pool, user, TokenPurpose::ResetPassword).await
    });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut reissue)
            .await
            .is_err(),
        "issuer must wait for the consuming account transaction"
    );

    // A consumer locking the token before its account would deadlock against reissue.
    tokio::time::timeout(
        std::time::Duration::from_secs(3),
        sqlx::query("UPDATE users SET password_hash='changed-test-hash' WHERE id=$1")
            .bind(user)
            .execute(&mut *consume),
    )
    .await
    .expect("account mutation must not deadlock")
    .unwrap();
    consume.commit().await.unwrap();

    let replacement = tokio::time::timeout(std::time::Duration::from_secs(3), reissue)
        .await
        .expect("issuer must complete after consumption commits")
        .unwrap()
        .unwrap();
    assert_ne!(raw, replacement);
    assert_eq!(
        consume_password_token(&pool, &replacement).await.unwrap(),
        user
    );
}
