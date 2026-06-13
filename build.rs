// Build script for generating gRPC code from protobuf definitions

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/");

    // Configure tonic-build
    // Note: tonic_build automatically writes to OUT_DIR, which is handled by tonic::include_proto!
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile(
            &[
                "proto/container.proto",
                "proto/network.proto",
                "proto/orchestration.proto",
            ],
            &["proto/"],
        )?;

    println!("cargo:warning=✅ gRPC code generated from protobuf definitions");

    Ok(())
}
