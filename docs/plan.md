# sw-ga144 development plan

The current implementation assembles F18A programs in X_eTaL and
executes them on a simulated GA144. The Rust emulator is the reference
for comparing future implementations. The browser demo shows node
registers, RAM, and port waits.

## Multiple chips

GA144 Studio previews a two-chip point-to-point link, a five-chip ring,
and an 8-by-8 mesh. Select a chip to inspect its 144 nodes. The preview
uses one generic package shape and displays the planned topology.

1. Connect two simulated GA144s through an explicit chip-to-chip
   transport. Verify a value transfer in each direction.
2. Use the same transport to connect chips in a ring. Verify forwarding
   around the ring.
3. Connect chips in a mesh and verify routes that cross several chips.

For richer 3D previews, evaluate generating an unmarked chip model
with the X_eTaL-extensions Blender extension and selecting instances
with the browser's Three.js extension. Use the same chip identifiers
for selection and the node inspector.

## Future TODOs

- **Development board.** Port the headless emulator to an Arduino UNO Q
  or a similar board. Load the same assembled programs and compare the
  results with the desktop emulator.
- **GPU.** Implement a GPU backend for the emulator. Start by running
  multiple chip instances in parallel, then compare node state and port
  transfers with the Rust reference when simulating connected chips.
- **FPGA.** Implement an F18A node and its port handshake in HDL.
  Validate single-node instruction traces, then compose the nodes into
  a GA144 simulation. Extend to a mesh of simulated chips as FPGA
  resources permit.
