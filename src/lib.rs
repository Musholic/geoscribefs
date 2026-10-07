pub mod proto {
    tonic::include_proto!("geoscribefs");
}

pub mod client;
pub mod fuse;
pub mod server;

pub const DEFAULT_ADDR: &str = "127.0.0.1:50051";
pub const TOKEN_ENV_VAR: &str = "GEOSCRIBE_TOKEN";
