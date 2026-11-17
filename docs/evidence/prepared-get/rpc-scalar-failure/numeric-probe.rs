fn main() {
 for x in [f32::MAX, f32::from_bits(1), f32::from_bits(0x3f7fffff), f32::from_bits(0x411fffff)] {
  let mut raw=serde_json::to_string(&x).unwrap();
  println!("f32bits={:08x} initial={}",x.to_bits(),raw);
  for n in 0..6 {
   let parsed:serde_json::Value=serde_json::from_str(&raw).unwrap();
   let encoded=serde_json::to_string(&parsed).unwrap();
   let decoded:f32=serde_json::from_str(&encoded).unwrap();
   println!("iteration={} raw={} next={} stable={} f32bits={:08x}",n,raw,encoded,raw==encoded,decoded.to_bits());
   raw=encoded;
  }
 }
}
