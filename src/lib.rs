pub mod proto {
    tonic::include_proto!("geoscribefs");
}

pub mod server;
pub mod client;


pub const DEFAULT_ADDR: &str = "127.0.0.1:50051";
pub const TOKEN_ENV_VAR: &str = "GEOSCRIBE_TOKEN";
