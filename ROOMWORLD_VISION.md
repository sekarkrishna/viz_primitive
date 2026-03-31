# Roomworld — A Spatial Desktop Environment

## The Idea

Replace the traditional OS desktop (windows, taskbars, start menus) with a navigable 2D space. You walk through rooms like Wolfenstein. Each room is a configurable space. Shortcuts are portals. No abstractions — just space you move through.

## Core Concepts

- Room: a rectangular space you can walk around in. Contains objects (files, apps, widgets).
- Portal: a doorway to another room. Click or walk through to navigate.
- Wall: room boundaries. Can display information (like a pinboard).
- Visitor: another person in your room. You see them, they see you.
- Public room: anyone can enter. Private room: only you. Password room: shared access.

## Rendering

2.5D raycasting — the Wolfenstein technique. No 3D engine, no linear algebra. Pure trigonometry.
- Cast one ray per screen column from the player's position
- Compute wall intersection distance using sin/cos/tan
- Draw vertical strips proportional to distance (closer = taller)
- dr2d renders the strips as colored SDF rectangles

This runs on the same GPU pipeline as justviz. The SDF renderer doesn't care if it's drawing chart points or wall strips.

## Networking — The Real Challenge

The rendering is the easy part. The hard part is connecting rooms across machines.

### Connection Primitive Analysis

Networking at the low level is mostly solved (TCP, QUIC, TLS). The real gaps are:

1. Peer-to-peer without infrastructure: connecting two machines directly without STUN/TURN servers. NAT traversal is still painful. libp2p exists but is complex.

2. Local-first sync: sharing state between machines without a cloud server. CRDTs solve the theory but tooling is immature. Automerge/Yjs exist for documents, not arbitrary app state.

3. Encrypted group communication: N people sharing a room with forward secrecy. MLS standard is new, implementations immature.

### Practical Approach

Start with the simplest thing that works: a central relay server (Rust binary that forwards messages). No P2P, no CRDT, no NAT traversal initially.

```
room_primitive (Rust)
  Phase 1: Central relay
    - WebSocket server, rooms as channels
    - JSON messages for state updates
    - Simple auth (room passwords)
  Phase 2: State sync
    - CRDT-based room state
    - Presence (who's online, who's where)
    - Visitor history
  Phase 3: Peer-to-peer
    - QUIC direct connections
    - NAT hole punching
    - Relay fallback when direct fails
```

## Architecture

```
dr2d (pixels)  +  room_primitive (bytes)
       |                    |
    roomworld
  |- Raycasting renderer (dr2d)
  |- Room state sync (room_primitive)
  |- Visitor presence (room_primitive)
  |- Portal navigation (dr2d + room_primitive)
  |- Window embedding (Wayland/X11 compositor — hardest part)
```

## Effort Estimate

- Raycasting renderer + room navigation demo: 1-2 months
- Central relay + basic multiplayer rooms: 2-3 months
- Window embedding (running real apps inside rooms): 6-12 months
- P2P networking + NAT traversal: 3-6 months
- Full DE replacement: multi-year

## Key Insight

The rendering primitive (dr2d) transfers directly. The networking primitive needs to be built. But start with a dumb relay — same philosophy as dr2d: start with the primitive, build up.

## Status

Vision document. Not started. Depends on dr2d being stable (it is) and a networking primitive being built.
