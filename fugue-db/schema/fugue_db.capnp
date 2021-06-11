@0xc9cbb65ef49adcf9;

struct Architecture { # format follows Ghidra's ldefs
  processor @0 :Text;
  endian @1 :Bool; # T if big, F if little
  bits @2 :UInt32;
  variant @3 :Text;
}

struct Segment {
  name @0 :Text;
  address @1 :UInt64;
  length @2 :UInt32;
  alignment @3 :UInt32;
  addressSize @4 :UInt32;
  code @5 :Bool;
  data @6 :Bool;
  external @7 :Bool;
  executable @8 :Bool;
  readable @9 :Bool;
  writable @10 :Bool;
  content @11 :Data;
}

struct InterRef { # Interprocedural edge
  address @0 :UInt64; # address of call site
  source @1 :UInt32 = 0xffffffff; # function; can be invalid (UINT32_MAX)
  target @2 :UInt32; # function
  call @3 :Bool; # is a call (or jump)
}

struct IntraRef { # Intraprocedural edge
  source @0 :UInt64; # block (id local to this graph)
  target @1 :UInt64; # block (id local to this graph)
  function @2 :UInt32; # function
}

struct BasicBlock {
  address @0 :UInt64;
  length @1 :UInt32;
  architecture @2 :UInt32 = 0; # Architecture ID
  predecessors @3 :List(IntraRef);
  successors @4 :List(IntraRef);
}

struct Function {
  symbol @0 :Text;
  entry @1 :UInt64; # block
  address @2 :UInt64;
  blocks @3 :List(BasicBlock);
  references @4 :List(InterRef);
}

struct ExportInfo {
  inputPath @0 :Text;
  inputMd5 @1 :Data; # Input file hash (MD5)
  inputSha256 @2 :Data; # Input file hash (SHA-256)
  startTime @3 :UInt64; # Time started exporter
  exportTime @4 :UInt64; # Time started actual export
  finishTime @5 :UInt64; # Time completed export
  exporter @6 :Text; # E.g., IDA Pro, Ghidra, etc.
  fileSize @7 :UInt64; # File size
}

struct Database {
  endian @0 :Bool; # T if big, F if little
  format @1 :Text = "Raw";
  architectures @2 :List(Architecture);
  segments @3 :List(Segment);
  functions @4 :List(Function);
  exportInfo @5 :ExportInfo;
}
