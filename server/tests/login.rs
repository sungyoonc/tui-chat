use test_util::spawn_server;
use serde::Serialize;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize)]
pub struct LoginData {
    pub username: String,
    pub pw: String,
    pub remember: bool,
}

#[tokio::test]
async fn test_login() {
    let (server_task, address, cancel_token) = spawn_server().await;
    let client = reqwest::Client::new();

    let map = LoginData {
        username: "my_id".to_string(),
        pw: "my_pw".to_string(),
        remember: true,
    };

    let response = client
        .post(format!("http://127.0.0.1:{}/auth/login", address.port()))
        .json(&map)
        .send()
        .await
        .expect("Failed to send request.");

    assert!(response.status().is_success());
    println!("{:?}", response.text().await);

    // Shutdown the server
    cancel_token.cancel();
    server_task.await.unwrap();
}

