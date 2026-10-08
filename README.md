# GeoScribeFS

> [!WARNING]
> This project is a Work In Progress (WIP). Most features are not yet implemented

GeoScribeFS is a distributed storage system designed to synchronize volumes transparently across multiple nodes, particularly in high-latency environments. It uses a dynamic single-writer model in which write authority can migrate between nodes on demand.

GeoScribeFS was originally started to support data sharing between nodes in a Docker Swarm. The goal is to let a service continue operating on another node if its current node goes down or the service migrates, with no or minimal data loss.

## Core Concept

The system uses the copy-on-write (CoW) snapshot capabilities of modern filesystems, such as Btrfs or ZFS, to synchronize data efficiently.

To maintain consistency across high-latency links, GeoScribeFS ensures that only one node, the **Scribe**, has write access to a volume at a time. A node without write access that attempts to modify the filesystem automatically requests authorization from a group of leader nodes. The current Scribe must relinquish write authority and return to read-only mode before the new node is granted permission to write.

## Key Features

- **Dynamic Single-Writer Model**:
  - Any node can attempt a write operation.
  - A node that is not the current Scribe requests write authorization through the leader nodes.
  - The current Scribe finishes its session and returns to read-only mode, allowing the requesting node to take over.
  - The active Scribe gets native filesystem read/write performance.
- **Transparent Read Consistency**: Non-writing nodes can read the volume with a guarantee that the returned data is up to date as of the start of the read operation. This is achieved by retrieving incremental snapshots on demand.
- **High-Latency Tolerance**: Snapshot-based synchronization minimizes the impact of network round trips on standard file operations.
- **Distributed Consensus**: A lightweight consensus mechanism among selected leader nodes manages the migration of write permissions.
- **FUSE-Powered Gating**: A custom FUSE overmount intercepts write attempts. Once write authorization is granted, the system transparently detaches the FUSE layer to provide native filesystem speed.
