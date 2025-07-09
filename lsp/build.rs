use std::io::Result;

fn main() -> Result<()> {
    // Compile protobuf files
    prost_build::compile_protos(&["src/proto/build.proto"], &["src/proto/"])?;
    
    Ok(())
} 