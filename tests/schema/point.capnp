@0xbf5147cbbecf40c1;
using Rust = import "/capnp/rust.capnp";
$Rust.parentModule("test_gen");

struct Point {
  x @0 :Int32;
  y @1 :Int32;
}
struct Line {
  start @0 :Point;
  end @1 :Point;
  label @2 :Text;
}
