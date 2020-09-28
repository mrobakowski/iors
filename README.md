# iors
`iors` (pronounce it however you want, but I like to say it like "yours") is an experimental IO runtime for Scala,
 written in Rust. Lots of JNI and other kinds of scary code.
 
 ## Performance
 Currently unknown. 
 
 When I first started the project, I thought it _may_ outperform an optimized IO interpreter written 
 in Scala because, you know, JVM is supposed to be slow and Rust is supposed to be fast. 
 **BUT**, after thinking about this more, this will obviously inhibit some of JVM's JIT optimizations, so it can 
 actually end up _slower_. Who knows.
 
 Benchmarks soon.
 
 ## Why?
 idk
 
 for fun?
 
 ## Should I use this?
 no.
 
 Not yet.
 
 ## License
 MIT