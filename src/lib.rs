#[allow(clippy::result_large_err)]
pub mod proto {
    tonic::include_proto!("geoscribefs");
}

pub mod cli;
pub mod client;
pub mod fuse;
pub mod server;
