# A first Work Process

From a drawing to a Work Process a node runs: a client's orders, collected
three at a time per client, marked and published to a Send Port — each
client's orders in the order they arrived, clients side by side
(`doc/architecture/runtime-model.md` sections 3 and 22; ADR-0066, amendment
2026-10-22).

## 1. The cluster: a correlation set, and an Application

In the cluster's `xmip.toml`, the set the work is spread over, declared once
for the cluster, and the Application that uses it:

```toml
[[correlation_sets]]
name = "PerClient"
keys = ["context:http.header.x-client"]   # the client, as a filter reads it
on_failure = "block"                       # the default; "skip" lets the rest pass

[[xmip_applications]]
name = "Orders"

[[xmip_applications.receive_ports]]
name = "Orders"

[[xmip_applications.receive_locations]]
name = "OrdersIn"
receive_port = "Orders"
interaction = "data-transfer"
depth = "context"
correlation_set = "PerClient"     # clients side by side, each client in order

[[xmip_applications.work_processes]]
name = "Batches"

[[xmip_applications.send_ports]]
name = "Out"

[[xmip_applications.subscriptions]]
id = "work"
destination = { work-process = "Batches" }
filter = "exists http.header.x-client and not exists Status"

[[xmip_applications.subscriptions]]
id = "out"
destination = { send-port = "Out" }
filter = "Status = 'batched'"
```

Without `correlation_set` on `OrdersIn`, the Location goes one Stream at a
time, end to end, keyed by its own name — what an update of all the
merchandise needs, and nothing to configure.

## 2. The design

Open `batches.bpmn` in VS Code: the Xmip Work Process designer draws it. A
client's first order starts its Instance; the next three are collected, the
batch marked and published. The file is standard BPMN 2.0 with its BPMN DI,
so any BPMN tool opens it; what Xmip adds is in BPMN's extension elements.

```xml
<?xml version="1.0" encoding="UTF-8"?>
<bpmn:definitions xmlns:bpmn="http://www.omg.org/spec/BPMN/20100524/MODEL"
    xmlns:bpmndi="http://www.omg.org/spec/BPMN/20100524/DI"
    xmlns:dc="http://www.omg.org/spec/DD/20100524/DC"
    xmlns:di="http://www.omg.org/spec/DD/20100524/DI"
    xmlns:xmip="urn:xmip:process:1" id="D" targetNamespace="urn:example:orders">
  <bpmn:process id="Batches" isExecutable="true">
    <bpmn:startEvent id="First">
      <bpmn:extensionElements><xmip:correlate set="PerClient"/></bpmn:extensionElements>
    </bpmn:startEvent>
    <bpmn:receiveTask id="Collect">
      <bpmn:extensionElements>
        <xmip:correlate set="PerClient"/>
        <xmip:batch most="3"/>
      </bpmn:extensionElements>
    </bpmn:receiveTask>
    <bpmn:task id="Mark">
      <bpmn:dataOutputAssociation id="Marked">
        <bpmn:assignment><bpmn:from>'batched'</bpmn:from><bpmn:to>Status</bpmn:to></bpmn:assignment>
      </bpmn:dataOutputAssociation>
    </bpmn:task>
    <bpmn:sendTask id="Publish"/>
    <bpmn:endEvent id="Done"/>
    <bpmn:sequenceFlow id="F1" sourceRef="First" targetRef="Collect"/>
    <bpmn:sequenceFlow id="F2" sourceRef="Collect" targetRef="Mark"/>
    <bpmn:sequenceFlow id="F3" sourceRef="Mark" targetRef="Publish"/>
    <bpmn:sequenceFlow id="F4" sourceRef="Publish" targetRef="Done"/>
  </bpmn:process>
  <bpmndi:BPMNDiagram id="Diagram">
    <bpmndi:BPMNPlane id="Plane" bpmnElement="Batches">
      <bpmndi:BPMNShape id="First_di" bpmnElement="First">
        <dc:Bounds x="40" y="80" width="36" height="36"/>
      </bpmndi:BPMNShape>
      <bpmndi:BPMNShape id="Collect_di" bpmnElement="Collect">
        <dc:Bounds x="120" y="58" width="100" height="80"/>
      </bpmndi:BPMNShape>
      <bpmndi:BPMNShape id="Mark_di" bpmnElement="Mark">
        <dc:Bounds x="270" y="58" width="100" height="80"/>
      </bpmndi:BPMNShape>
      <bpmndi:BPMNShape id="Publish_di" bpmnElement="Publish">
        <dc:Bounds x="420" y="58" width="100" height="80"/>
      </bpmndi:BPMNShape>
      <bpmndi:BPMNShape id="Done_di" bpmnElement="Done">
        <dc:Bounds x="570" y="80" width="36" height="36"/>
      </bpmndi:BPMNShape>
      <bpmndi:BPMNEdge id="F1_di" bpmnElement="F1">
        <di:waypoint x="76" y="98"/><di:waypoint x="120" y="98"/>
      </bpmndi:BPMNEdge>
      <bpmndi:BPMNEdge id="F2_di" bpmnElement="F2">
        <di:waypoint x="220" y="98"/><di:waypoint x="270" y="98"/>
      </bpmndi:BPMNEdge>
      <bpmndi:BPMNEdge id="F3_di" bpmnElement="F3">
        <di:waypoint x="370" y="98"/><di:waypoint x="420" y="98"/>
      </bpmndi:BPMNEdge>
      <bpmndi:BPMNEdge id="F4_di" bpmnElement="F4">
        <di:waypoint x="520" y="98"/><di:waypoint x="570" y="98"/>
      </bpmndi:BPMNEdge>
    </bpmndi:BPMNPlane>
  </bpmndi:BPMNDiagram>
</bpmn:definitions>
```

Nothing in the design is fetched: a namespace is a name, and a design naming a
document on a network is refused before anything is asked.

## 3. Compile it

In the designer, **Compile**: it asks for the module's name — `batches` — and
writes the unit beside the design, in `batches/`: its `Cargo.toml`, its
`src/lib.rs` carrying the compiled program, and `module.toml`, its
`[[modules]]` entry. A refusal is said in words naming the element, and
nothing is written. Then build it as Xmip is built, on the machine and for the
target the node runs on:

```powershell
cargo build --release --manifest-path batches/Cargo.toml
```

The library — `batches.dll`, `libbatches.so` or `libbatches.dylib` — is the
Work Process unit. Copy it where the node's configuration says.

## 4. The node: the module, and the Work Process naming its unit

`module.toml`, as the compiler wrote it, goes into the node's configuration
with the Work Process that names it:

```toml
[[modules]]
name = "batches"
start = true

[modules.manifest]
offline = true

[modules.manifest.identity]
name = "batches"
version = "0.1.0"

[[modules.manifest.capabilities]]
capability = "work-process"
execution_host = "native-rust"
trusted_required = false

[modules.manifest.entrypoint]
library_path = "batches.dll"
symbol = "xmip_create_module_v1"

[[modules.manifest.units]]
name = "Batches"
capability = "work-process"
stages = ["process"]
offline = true

[[work_processes]]
name = "Batches"
start = true
module = "batches"
unit = "Batches"
```

The node opens the library as it starts, through the C ABI
(`xmip_module.h` section 13), and its Work Process step takes a turn of each
client's Instance as its orders arrive. An Instance waiting for the rest of
its batch is a record in the Ledger, holding no thread; a node killed
meanwhile, or another node taking its place, goes on from it.

The same design runs in `xmip-core-runtime`'s `tests/ledger/work_process.rs`.
