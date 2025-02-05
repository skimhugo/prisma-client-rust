use prisma_client_rust::{prisma_models::PrismaValue, raw, BatchResult};

use crate::{db::*, utils::*};

#[tokio::test]
async fn query_raw() -> TestResult {
    let client = client().await;

    client
        .post()
        .create(
            post::title::set("My post title!".to_string()),
            post::published::set(false),
            vec![],
        )
        .exec()
        .await?;

    let result: Vec<BatchResult> = client
        ._query_raw(raw!("SELECT COUNT(*) as count FROM Post"))
        .await?;
    assert_eq!(result.len(), 1);

    cleanup(client).await
}

#[tokio::test]
async fn query_raw_model() -> TestResult {
    let client = client().await;

    let post = client
        .post()
        .create(
            post::title::set("My post title!".to_string()),
            post::published::set(false),
            vec![],
        )
        .exec()
        .await?;

    let result: Vec<post::Data> = client
        ._query_raw(raw!(
            "SELECT * FROM Post WHERE id = {}",
            PrismaValue::String(post.id.clone())
        ))
        .await?;
    assert_eq!(result.len(), 1);
    assert_eq!(&result[0].id, &post.id);
    assert_eq!(result[0].published, false);

    cleanup(client).await
}

#[tokio::test]
async fn query_raw_no_result() -> TestResult {
    let client = client().await;

    let result: Vec<post::Data> = client
        ._query_raw(raw!("SELECT * FROM Post WHERE id = 'sdldsd'"))
        .await?;
    assert_eq!(result.len(), 0);

    cleanup(client).await
}

#[tokio::test]
async fn execute_raw() -> TestResult {
    let client = client().await;

    let post = client
        .post()
        .create(
            post::title::set("My post title!".to_string()),
            post::published::set(false),
            vec![],
        )
        .exec()
        .await?;

    let count = client
        ._execute_raw(raw!(
            "UPDATE Post SET title = 'My edited title' WHERE id = {}",
            PrismaValue::String(post.id.clone())
        ))
        .await?;
    assert_eq!(count, 1);

    let found = client
        .post()
        .find_unique(post::id::equals(post.id.clone()))
        .exec()
        .await?;
    assert!(found.is_some());
    let found = found.unwrap();
    assert_eq!(&found.id, &post.id);
    assert_eq!(&found.title, "My edited title");

    cleanup(client).await
}

#[tokio::test]
async fn execute_raw_no_result() -> TestResult {
    let client = client().await;

    let count = client
        ._execute_raw(raw!(
            "UPDATE Post SET title = 'updated title' WHERE id = 'sdldsd'"
        ))
        .await?;
    assert_eq!(count, 0);

    cleanup(client).await
}

// query_first?