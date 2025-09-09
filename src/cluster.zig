const std = @import("std");
const QuicFabric = @import("quic_fabric.zig");
const ContentStore = @import("content_store.zig");
const Migration = @import("migration.zig");

pub const ClusterError = error{
    NodeUnreachable,
    ClusterSplitBrain,
    InsufficientResources,
    SchedulingFailed,
    ConsensusTimeout,
    InvalidNodeState,
};

pub const NodeState = enum {
    joining,
    active,
    draining,
    failed,
    maintenance,
};

pub const NodeResources = struct {
    cpu_cores: u32,
    memory_gb: u32,
    storage_gb: u32,
    network_bandwidth_mbps: u32,
    
    // Current usage
    cpu_used: f32 = 0.0,
    memory_used_gb: u32 = 0,
    storage_used_gb: u32 = 0,
    
    pub fn availableCPU(self: NodeResources) f32 {
        return @as(f32, @floatFromInt(self.cpu_cores)) - self.cpu_used;
    }
    
    pub fn availableMemory(self: NodeResources) u32 {
        return self.memory_gb - self.memory_used_gb;
    }
    
    pub fn availableStorage(self: NodeResources) u32 {
        return self.storage_gb - self.storage_used_gb;
    }
    
    pub fn canSchedule(self: NodeResources, required_cpu: f32, required_memory: u32, required_storage: u32) bool {
        return self.availableCPU() >= required_cpu and 
               self.availableMemory() >= required_memory and 
               self.availableStorage() >= required_storage;
    }
};

pub const ClusterNode = struct {
    id: []const u8,
    address: []const u8,
    port: u16,
    state: NodeState,
    resources: NodeResources,
    last_heartbeat: i64,
    metadata: std.StringHashMap([]const u8),
    running_capsules: std.StringHashMap(CapsuleAssignment),
    
    pub const CapsuleAssignment = struct {
        capsule_id: []const u8,
        cpu_allocated: f32,
        memory_allocated: u32,
        storage_allocated: u32,
        
        pub fn deinit(self: *CapsuleAssignment, allocator: std.mem.Allocator) void {
            allocator.free(self.capsule_id);
        }
    };
    
    pub fn init(allocator: std.mem.Allocator, id: []const u8, address: []const u8, port: u16, resources: NodeResources) !ClusterNode {
        return ClusterNode{
            .id = try allocator.dupe(u8, id),
            .address = try allocator.dupe(u8, address),
            .port = port,
            .state = .joining,
            .resources = resources,
            .last_heartbeat = std.time.timestamp(),
            .metadata = std.StringHashMap([]const u8).init(allocator),
            .running_capsules = std.StringHashMap(CapsuleAssignment).init(allocator),
        };
    }
    
    pub fn deinit(self: *ClusterNode, allocator: std.mem.Allocator) void {
        allocator.free(self.id);
        allocator.free(self.address);
        
        var metadata_iter = self.metadata.iterator();
        while (metadata_iter.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.*);
        }
        self.metadata.deinit();
        
        var capsule_iter = self.running_capsules.iterator();
        while (capsule_iter.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            var assignment = entry.value_ptr;
            assignment.deinit(allocator);
        }
        self.running_capsules.deinit();
    }
    
    pub fn isHealthy(self: *const ClusterNode) bool {
        const now = std.time.timestamp();
        const heartbeat_threshold = 30; // 30 seconds
        
        return self.state == .active and 
               (now - self.last_heartbeat) < heartbeat_threshold;
    }
    
    pub fn assignCapsule(self: *ClusterNode, allocator: std.mem.Allocator, capsule_id: []const u8, cpu: f32, memory: u32, storage: u32) !void {
        if (!self.resources.canSchedule(cpu, memory, storage)) {
            return ClusterError.InsufficientResources;
        }
        
        const assignment = CapsuleAssignment{
            .capsule_id = try allocator.dupe(u8, capsule_id),
            .cpu_allocated = cpu,
            .memory_allocated = memory,
            .storage_allocated = storage,
        };
        
        const capsule_key = try allocator.dupe(u8, capsule_id);
        try self.running_capsules.put(capsule_key, assignment);
        
        // Update resource usage
        self.resources.cpu_used += cpu;
        self.resources.memory_used_gb += memory;
        self.resources.storage_used_gb += storage;
        
        std.debug.print("[Cluster] Assigned capsule {s} to node {s}\n", .{ capsule_id, self.id });
    }
    
    pub fn unassignCapsule(self: *ClusterNode, capsule_id: []const u8) void {
        if (self.running_capsules.fetchRemove(capsule_id)) |entry| {
            const assignment = entry.value;
            
            // Free resources
            self.resources.cpu_used -= assignment.cpu_allocated;
            self.resources.memory_used_gb -= assignment.memory_allocated;
            self.resources.storage_used_gb -= assignment.storage_allocated;
            
            std.debug.print("[Cluster] Unassigned capsule {s} from node {s}\n", .{ capsule_id, self.id });
        }
    }
};

pub const SchedulingPolicy = enum {
    round_robin,
    least_loaded,
    resource_balanced,
    affinity_aware,
};

pub const PlacementConstraint = struct {
    required_labels: std.StringHashMap([]const u8),
    anti_affinity: [][]const u8, // Don't place on same node as these capsules
    preferred_nodes: [][]const u8,
    min_cpu: f32,
    min_memory: u32,
    min_storage: u32,
    
    pub fn deinit(self: *PlacementConstraint, allocator: std.mem.Allocator) void {
        var label_iter = self.required_labels.iterator();
        while (label_iter.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.*);
        }
        self.required_labels.deinit();
        
        for (self.anti_affinity) |capsule_id| {
            allocator.free(capsule_id);
        }
        allocator.free(self.anti_affinity);
        
        for (self.preferred_nodes) |node_id| {
            allocator.free(node_id);
        }
        allocator.free(self.preferred_nodes);
    }
};

pub const SurgeCluster = struct {
    allocator: std.mem.Allocator,
    local_node_id: []const u8,
    nodes: std.StringHashMap(ClusterNode),
    quic_fabric: *QuicFabric.QuicFabric,
    content_store: *ContentStore.ContentAddressedStore,
    migration_manager: *Migration.MigrationManager,
    leader_node: ?[]const u8,
    cluster_state: ClusterState,
    
    const ClusterState = enum {
        bootstrapping,
        active,
        degraded,
        maintenance,
    };
    
    pub fn init(allocator: std.mem.Allocator, node_id: []const u8, quic_fabric: *QuicFabric.QuicFabric, 
               content_store: *ContentStore.ContentAddressedStore, migration_manager: *Migration.MigrationManager) !SurgeCluster {
        return SurgeCluster{
            .allocator = allocator,
            .local_node_id = try allocator.dupe(u8, node_id),
            .nodes = std.StringHashMap(ClusterNode).init(allocator),
            .quic_fabric = quic_fabric,
            .content_store = content_store,
            .migration_manager = migration_manager,
            .leader_node = null,
            .cluster_state = .bootstrapping,
        };
    }
    
    pub fn deinit(self: *SurgeCluster) void {
        var node_iter = self.nodes.iterator();
        while (node_iter.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            var node = entry.value_ptr;
            node.deinit(self.allocator);
        }
        self.nodes.deinit();
        self.allocator.free(self.local_node_id);
    }
    
    pub fn joinCluster(self: *SurgeCluster, bootstrap_nodes: [][]const u8) !void {
        std.debug.print("[Cluster] Joining cluster with bootstrap nodes\n");
        
        // Add local node
        const local_resources = NodeResources{
            .cpu_cores = 8,
            .memory_gb = 32,
            .storage_gb = 500,
            .network_bandwidth_mbps = 1000,
        };
        
        var local_node = try ClusterNode.init(self.allocator, self.local_node_id, "127.0.0.1", 4433, local_resources);
        local_node.state = .active;
        
        const local_key = try self.allocator.dupe(u8, self.local_node_id);
        try self.nodes.put(local_key, local_node);
        
        // Connect to bootstrap nodes
        for (bootstrap_nodes) |bootstrap_node| {
            self.connectToNode(bootstrap_node) catch |err| {
                std.debug.print("[Cluster] Failed to connect to bootstrap node {s}: {any}\n", .{ bootstrap_node, err });
            };
        }
        
        // Start consensus process if no leader
        if (self.leader_node == null) {
            try self.startLeaderElection();
        }
        
        self.cluster_state = .active;
        std.debug.print("[Cluster] Successfully joined cluster\n");
    }
    
    pub fn scheduleCapsule(self: *SurgeCluster, capsule_id: []const u8, constraints: PlacementConstraint, policy: SchedulingPolicy) ![]const u8 {
        std.debug.print("[Cluster] Scheduling capsule: {s}\n", .{capsule_id});
        
        const selected_node = try self.selectNode(constraints, policy);
        
        // Assign capsule to selected node
        if (self.nodes.getPtr(selected_node)) |node| {
            try node.assignCapsule(self.allocator, capsule_id, constraints.min_cpu, constraints.min_memory, constraints.min_storage);
            
            // If not local node, trigger remote deployment
            if (!std.mem.eql(u8, selected_node, self.local_node_id)) {
                try self.deployRemoteCapsule(capsule_id, selected_node);
            }
            
            std.debug.print("[Cluster] Scheduled capsule {s} on node {s}\n", .{ capsule_id, selected_node });
            return selected_node;
        }
        
        return ClusterError.SchedulingFailed;
    }
    
    pub fn rebalanceCluster(self: *SurgeCluster) !void {
        std.debug.print("[Cluster] Starting cluster rebalancing\n");
        
        // Calculate cluster resource utilization
        var total_cpu: f32 = 0;
        var used_cpu: f32 = 0;
        var overloaded_nodes = std.ArrayList([]const u8).init(self.allocator);
        defer overloaded_nodes.deinit();
        
        var node_iter = self.nodes.iterator();
        while (node_iter.next()) |entry| {
            const node = entry.value_ptr;
            if (!node.isHealthy()) continue;
            
            total_cpu += @as(f32, @floatFromInt(node.resources.cpu_cores));
            used_cpu += node.resources.cpu_used;
            
            // Mark nodes over 80% CPU utilization as overloaded
            if (node.resources.cpu_used / @as(f32, @floatFromInt(node.resources.cpu_cores)) > 0.8) {
                try overloaded_nodes.append(entry.key_ptr.*);
            }
        }
        
        const cluster_utilization = used_cpu / total_cpu;
        std.debug.print("[Cluster] Cluster CPU utilization: {d:.2}%\n", .{cluster_utilization * 100});
        
        // Migrate capsules from overloaded nodes
        for (overloaded_nodes.items) |overloaded_node_id| {
            if (self.nodes.get(overloaded_node_id)) |overloaded_node| {
                try self.migrateFromOverloadedNode(overloaded_node_id, &overloaded_node);
            }
        }
        
        std.debug.print("[Cluster] Cluster rebalancing completed\n");
    }
    
    pub fn handleNodeFailure(self: *SurgeCluster, failed_node_id: []const u8) !void {
        std.debug.print("[Cluster] Handling node failure: {s}\n", .{failed_node_id});
        
        if (self.nodes.getPtr(failed_node_id)) |failed_node| {
            failed_node.state = .failed;
            
            // Reschedule all capsules from failed node
            var capsule_iter = failed_node.running_capsules.iterator();
            while (capsule_iter.next()) |entry| {
                const capsule_id = entry.key_ptr.*;
                const assignment = entry.value_ptr;
                
                // Create placement constraints from original assignment
                var constraints = PlacementConstraint{
                    .required_labels = std.StringHashMap([]const u8).init(self.allocator),
                    .anti_affinity = &[_][]const u8{},
                    .preferred_nodes = &[_][]const u8{},
                    .min_cpu = assignment.cpu_allocated,
                    .min_memory = assignment.memory_allocated,
                    .min_storage = assignment.storage_allocated,
                };
                defer constraints.deinit(self.allocator);
                
                // Try to reschedule
                const new_node_id = self.scheduleCapsule(capsule_id, constraints, .least_loaded) catch |err| {
                    std.debug.print("[Cluster] Failed to reschedule capsule {s}: {any}\n", .{ capsule_id, err });
                    continue;
                };
                
                std.debug.print("[Cluster] Rescheduled capsule {s} from failed node {s} to {s}\n", 
                    .{ capsule_id, failed_node_id, new_node_id });
            }
            
            // Clear assignments from failed node
            failed_node.running_capsules.clearAndFree();
            
            // If failed node was leader, start new election
            if (self.leader_node) |leader| {
                if (std.mem.eql(u8, leader, failed_node_id)) {
                    self.leader_node = null;
                    try self.startLeaderElection();
                }
            }
        }
        
        std.debug.print("[Cluster] Node failure handling completed: {s}\n", .{failed_node_id});
    }
    
    // Private implementation methods
    fn connectToNode(self: *SurgeCluster, node_address: []const u8) !void {
        std.debug.print("[Cluster] Connecting to node: {s}\n", .{node_address});
        
        // Parse node address (id@address:port)
        var parts = std.mem.splitSequence(u8, node_address, "@");
        const node_id = parts.next() orelse return ClusterError.InvalidNodeState;
        const addr_port = parts.next() orelse return ClusterError.InvalidNodeState;
        
        var addr_parts = std.mem.splitSequence(u8, addr_port, ":");
        const address = addr_parts.next() orelse return ClusterError.InvalidNodeState;
        const port_str = addr_parts.next() orelse "4433";
        const port = try std.fmt.parseInt(u16, port_str, 10);
        
        // Create remote node entry
        const remote_resources = NodeResources{
            .cpu_cores = 8,  // Will be updated via heartbeat
            .memory_gb = 32,
            .storage_gb = 500,
            .network_bandwidth_mbps = 1000,
        };
        
        const remote_node = try ClusterNode.init(self.allocator, node_id, address, port, remote_resources);
        const node_key = try self.allocator.dupe(u8, node_id);
        try self.nodes.put(node_key, remote_node);
        
        // Register with QUIC fabric for communication
        try self.quic_fabric.registerLocalService(node_id, port);
        
        std.debug.print("[Cluster] Connected to node: {s}\n", .{node_id});
    }
    
    fn selectNode(self: *SurgeCluster, constraints: PlacementConstraint, policy: SchedulingPolicy) ![]const u8 {
        var candidate_nodes = std.ArrayList([]const u8).init(self.allocator);
        defer candidate_nodes.deinit();
        
        // Filter nodes based on constraints
        var node_iter = self.nodes.iterator();
        while (node_iter.next()) |entry| {
            const node_id = entry.key_ptr.*;
            const node = entry.value_ptr;
            
            if (!node.isHealthy()) continue;
            if (!node.resources.canSchedule(constraints.min_cpu, constraints.min_memory, constraints.min_storage)) continue;
            
            // Check anti-affinity constraints
            var violates_anti_affinity = false;
            for (constraints.anti_affinity) |anti_capsule| {
                if (node.running_capsules.contains(anti_capsule)) {
                    violates_anti_affinity = true;
                    break;
                }
            }
            if (violates_anti_affinity) continue;
            
            try candidate_nodes.append(node_id);
        }
        
        if (candidate_nodes.items.len == 0) {
            return ClusterError.InsufficientResources;
        }
        
        // Apply scheduling policy
        return switch (policy) {
            .round_robin => self.selectRoundRobin(candidate_nodes.items),
            .least_loaded => self.selectLeastLoaded(candidate_nodes.items),
            .resource_balanced => self.selectResourceBalanced(candidate_nodes.items),
            .affinity_aware => self.selectAffinityAware(candidate_nodes.items, constraints),
        };
    }
    
    fn selectLeastLoaded(self: *SurgeCluster, candidates: [][]const u8) []const u8 {
        var best_node: []const u8 = candidates[0];
        var lowest_utilization: f32 = 1.0;
        
        for (candidates) |node_id| {
            if (self.nodes.get(node_id)) |node| {
                const utilization = node.resources.cpu_used / @as(f32, @floatFromInt(node.resources.cpu_cores));
                if (utilization < lowest_utilization) {
                    lowest_utilization = utilization;
                    best_node = node_id;
                }
            }
        }
        
        return best_node;
    }
    
    fn selectRoundRobin(_: *SurgeCluster, candidates: [][]const u8) []const u8 {
        // Simple round-robin based on timestamp
        const index = @as(usize, @intCast(std.time.timestamp())) % candidates.len;
        return candidates[index];
    }
    
    fn selectResourceBalanced(self: *SurgeCluster, candidates: [][]const u8) []const u8 {
        // Select node with most balanced resource utilization
        var best_node: []const u8 = candidates[0];
        var best_balance: f32 = 1000.0;
        
        for (candidates) |node_id| {
            if (self.nodes.get(node_id)) |node| {
                const cpu_util = node.resources.cpu_used / @as(f32, @floatFromInt(node.resources.cpu_cores));
                const mem_util = @as(f32, @floatFromInt(node.resources.memory_used_gb)) / @as(f32, @floatFromInt(node.resources.memory_gb));
                const storage_util = @as(f32, @floatFromInt(node.resources.storage_used_gb)) / @as(f32, @floatFromInt(node.resources.storage_gb));
                
                // Calculate variance in resource utilization (lower is better)
                const mean = (cpu_util + mem_util + storage_util) / 3.0;
                const variance = ((cpu_util - mean) * (cpu_util - mean) + 
                                (mem_util - mean) * (mem_util - mean) + 
                                (storage_util - mean) * (storage_util - mean)) / 3.0;
                
                if (variance < best_balance) {
                    best_balance = variance;
                    best_node = node_id;
                }
            }
        }
        
        return best_node;
    }
    
    fn selectAffinityAware(self: *SurgeCluster, candidates: [][]const u8, constraints: PlacementConstraint) []const u8 {
        // Prefer nodes in the preferred_nodes list
        for (constraints.preferred_nodes) |preferred| {
            for (candidates) |candidate| {
                if (std.mem.eql(u8, candidate, preferred)) {
                    return candidate;
                }
            }
        }
        
        // Fall back to least loaded
        return self.selectLeastLoaded(candidates);
    }
    
    fn deployRemoteCapsule(self: *SurgeCluster, capsule_id: []const u8, target_node: []const u8) !void {
        std.debug.print("[Cluster] Deploying capsule {s} to remote node {s}\n", .{ capsule_id, target_node });
        
        // Send deployment message via QUIC fabric
        const deployment_msg = try std.fmt.allocPrint(self.allocator, 
            "DEPLOY_CAPSULE:{s}", .{capsule_id});
        defer self.allocator.free(deployment_msg);
        
        try self.quic_fabric.sendEncryptedMessage(target_node, deployment_msg);
        
        std.debug.print("[Cluster] Remote deployment initiated: {s} -> {s}\n", .{ capsule_id, target_node });
    }
    
    fn startLeaderElection(self: *SurgeCluster) !void {
        std.debug.print("[Cluster] Starting leader election\n");
        
        // Simple leader election: node with lowest lexicographic ID becomes leader
        var lowest_node_id: ?[]const u8 = null;
        
        var node_iter = self.nodes.iterator();
        while (node_iter.next()) |entry| {
            const node_id = entry.key_ptr.*;
            const node = entry.value_ptr;
            
            if (!node.isHealthy()) continue;
            
            if (lowest_node_id == null or std.mem.order(u8, node_id, lowest_node_id.?) == .lt) {
                lowest_node_id = node_id;
            }
        }
        
        if (lowest_node_id) |leader| {
            self.leader_node = leader;
            std.debug.print("[Cluster] New leader elected: {s}\n", .{leader});
        }
    }
    
    fn migrateFromOverloadedNode(self: *SurgeCluster, overloaded_node_id: []const u8, overloaded_node: *const ClusterNode) !void {
        std.debug.print("[Cluster] Migrating capsules from overloaded node: {s}\n", .{overloaded_node_id});
        
        var capsule_iter = overloaded_node.running_capsules.iterator();
        while (capsule_iter.next()) |entry| {
            const capsule_id = entry.key_ptr.*;
            const assignment = entry.value_ptr;
            
            // Find a less loaded node for migration
            const target_node = blk: {
                var node_iter = self.nodes.iterator();
                while (node_iter.next()) |node_entry| {
                    const node_id = node_entry.key_ptr.*;
                    const node = node_entry.value_ptr;
                    
                    if (std.mem.eql(u8, node_id, overloaded_node_id)) continue;
                    if (!node.isHealthy()) continue;
                    
                    const utilization = node.resources.cpu_used / @as(f32, @floatFromInt(node.resources.cpu_cores));
                    if (utilization < 0.5 and node.resources.canSchedule(assignment.cpu_allocated, assignment.memory_allocated, assignment.storage_allocated)) {
                        break :blk node_id;
                    }
                }
                break :blk null;
            };
            
            if (target_node) |target| {
                // Perform live migration
                std.debug.print("[Cluster] Migrating capsule {s} from {s} to {s}\n", 
                    .{ capsule_id, overloaded_node_id, target });
                
                // This would trigger the actual live migration process
                // For now, just log the intention
            }
        }
    }
};