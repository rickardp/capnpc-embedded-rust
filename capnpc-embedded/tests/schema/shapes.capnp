@0xdeadbeef12345678;
using Rust = import "/capnp/rust.capnp";
$Rust.parentModule("gen");
using T = import "types.capnp";

struct Circle {
  radius @0 :Float64;
  fill   @1 :T.Color;
}
