use super::super::client::AiGateway;

#[test]
fn gateway_clients_initialize_without_contacting_a_provider() {
    AiGateway::new().expect("AI gateway clients should initialize without network access");
}
