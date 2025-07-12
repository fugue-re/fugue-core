Diagram of Fugue's database, for comparing it to e.g. Ghidra.

Note of warning: [cardinality](https://mermaid.js.org/syntax/classDiagram.html#cardinality-multiplicity-on-relations) of connections might not be correct.

- Composition (filled diamond) means that the child is inside the parent data structure.
- Aggregation (unfilled diamond) means that the value stored can be used to find the child (like for example an address or an ID).

Based on the original revision of the database schema: https://github.com/fugue-re/fugue-db-schema/blob/10419ab5a06ac9ccbb4f2c101ff725b92e2ea389/fugue.fbs

```mermaid
classDiagram
    note for Project "Root struct.
    All indices are uint32"
    class Project {
      +Architecture[] architectures
      +Segment[] segments
      +Function[] functions
      +Metadata metadata
      +uint8[] auxiliary
    }

    note for Segment "Tagged address range.
    Address fields may be looked up here;
    connections not shown"
    class Segment {
      +string name
      +uint64 address
      +uint32 size
      +uint32 address_size
      +uint32 alignment
      +uint32 bits
      +bool endian
      +bool code
      +bool data
      +bool external
      +bool readable
      +bool writable
      +bool executable
      +uint8[] bytes
      +uint8[] auxiliary
    }

    note for IntraRef "Reference between basic blocks (within the same function)"
    class IntraRef {
      +uint64 source : block ID: 32 bits of function ID and 32 bits of block index in function
      +uint64 target : block ID: 32 bits of function ID and 32 bits of block index in function
      +uint32 function : ID of the function these basic blocks are contained in
      +uint8[] auxiliary
    }

    class BasicBlock {
      +uint64 address
      +uint32 size : size of the basic block's address range in bytes
      +uint32 architecture : architecture ID
      +IntraRef[] predecessors : which basic blocks lead to this basic block
      +IntraRef[] successors : which basic blocks this basic block leads to
      +uint8[] auxiliary
    }

    class Function {
      +string symbol : the name of the function
      +uint64 address : entrypoint address
      %% The block index for the entrypoint should always be 0, right?
      +uint64 entry : block ID: 32 bits of function ID and 32 bits of block index in function
      +BasicBlock[] blocks : basic blocks in the body of this function
      +InterRef[] references : references to this function from elsewhere
      +uint8[] auxiliary
    }

    note for InterRef "Reference between functions"
    class InterRef {
      +uint64 address : the address of the reference in the source function
      +uint32 source : function ID of the caller
      +uint32 target : function ID of the callee
      +bool call : whether the reference is a call or a jump
      +uint8[] auxiliary
    }

    class Metadata {
      +string input_format
      +string input_path
      +uint8[] input_md5
      +uint8[] input_sha256
      +uint32 input_size
      +string exporter
      +uint8[] auxiliary
    }

    note for Architecture "Definition of a processor architecture"
    class Architecture {
      +string processor
      +bool endian
      +uint32 bits
      +string variant
      +uint8[] auxiliary
    }

    Project "1" *-- "0..*" Architecture : architectures
    Project "1" *-- "0..*" Segment : segments
    Project "1" *-- "0..*" Function : functions
    Project "1" *-- "1" Metadata : metadata

    BasicBlock "1" *-- "0..*" IntraRef : predecessors,successors
    BasicBlock "1" o-- "0..*" Architecture : architecture
    IntraRef "0..*" o-- "2" BasicBlock : source,target
    IntraRef "0..*" o-- "0..*" Function : function

    Function "1" *-- "0..*" BasicBlock : blocks
    Function "1" o-- "1" BasicBlock : entry
    %% Has to be reversed so the graph stays clean
    InterRef "0..*" --* "1" Function : references
    InterRef "0..*" o-- "2" Function : source,target
```
