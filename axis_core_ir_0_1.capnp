@0xf8c9d8e7b6a5f4e3;
# Axis Core IR 0.1 - Binary Serialization Schema
# Data-only representation of Core calculus terms

struct CoreBundle {
  version @0 :Text;           # Must be "0.1"
  entrypointName @1 :Text;     # Name of entry function (documentation)
  entrypointId @2 :UInt32;     # Unambiguous entrypoint term ID
  stringTable @3 :List(Text);  # String literals and identifiers
  coreTerm @4 :CoreTerm;       # Root term graph
}

struct Span {
  file @0 :Text;
  line @1 :UInt32;
  column @2 :UInt32;
}

struct CoreTerm {
  union {
    cIntLit @0 :CIntLit;
    cBoolLit @1 :CBoolLit;
    cUnitLit @2 :CUnitLit;
    cStrLit @3 :CStrLit;
    cVar @4 :CVar;
    cLam @5 :CLam;
    cApp @6 :CApp;
    cTuple @7 :CTuple;
    cProj @8 :CProj;
    cLet @9 :CLet;
    cIf @10 :CIf;
    cCtor @11 :CCtor;
    cMatch @12 :CMatch;
  }
}

struct CIntLit {
  value @0 :Int64;
  span @1 :Span;
}

struct CBoolLit {
  value @0 :Bool;
  span @1 :Span;
}

struct CUnitLit {
  span @0 :Span;
}

struct CStrLit {
  value @0 :Text;
  span @1 :Span;
}

struct CVar {
  name @0 :Text;
  span @1 :Span;
}

struct CLam {
  param @0 :Text;
  body @1 :CoreTerm;
  span @2 :Span;
}

struct CApp {
  func @0 :CoreTerm;
  arg @1 :CoreTerm;
  span @2 :Span;
}

struct CTuple {
  elems @0 :List(CoreTerm);
  span @1 :Span;
}

struct CProj {
  expr @0 :CoreTerm;
  index @1 :UInt32;
  span @2 :Span;
}

struct CLet {
  name @0 :Text;
  value @1 :CoreTerm;
  body @2 :CoreTerm;
  span @3 :Span;
}

struct CIf {
  cond @0 :CoreTerm;
  thenBranch @1 :CoreTerm;
  elseBranch @2 :CoreTerm;
  span @3 :Span;
}

struct CCtor {
  name @0 :Text;
  fields @1 :List(CoreTerm);
  span @2 :Span;
}

struct CMatch {
  scrutinee @0 :CoreTerm;
  arms @1 :List(MatchArm);
  span @2 :Span;
}

struct MatchArm {
  pattern @0 :Pattern;
  body @1 :CoreTerm;
}

struct Pattern {
  union {
    pInt @0 :PInt;
    pBool @1 :PBool;
    pUnit @2 :PUnit;
    pVar @3 :PVar;
    pTuple @4 :PTuple;
    pEnum @5 :PEnum;
  }
}

struct PInt {
  value @0 :Int64;
}

struct PBool {
  value @0 :Bool;
}

struct PUnit {}

struct PVar {
  name @0 :Text;
}

struct PTuple {
  patterns @0 :List(Pattern);
}

struct PEnum {
  name @0 :Text;
  patterns @1 :List(Pattern);
}
