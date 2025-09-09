const std = @import("std");
const linux = std.os.linux;

pub const NetworkError = error{
    BridgeCreationFailed,
    InterfaceCreationFailed,
    AddressAssignmentFailed,
    RoutingFailed,
    PermissionDenied,
};

pub const NetworkType = enum {
    bridge,
    host,
    none,
};

pub const NetworkConfig = struct {
    name: []const u8,
    network_type: NetworkType,
    subnet: ?[]const u8 = null,
    gateway: ?[]const u8 = null,
    dns_servers: [][]const u8 = &.{},
};

pub const BridgeNetwork = struct {
    allocator: std.mem.Allocator,
    name: []const u8,
    subnet: []const u8,
    gateway: []const u8,
    created: bool = false,
    
    pub fn init(allocator: std.mem.Allocator, config: NetworkConfig) !BridgeNetwork {
        return BridgeNetwork{
            .allocator = allocator,
            .name = try allocator.dupe(u8, config.name),
            .subnet = try allocator.dupe(u8, config.subnet orelse "172.17.0.0/16"),
            .gateway = try allocator.dupe(u8, config.gateway orelse "172.17.0.1"),
        };
    }
    
    pub fn deinit(self: *BridgeNetwork) void {
        self.allocator.free(self.name);
        self.allocator.free(self.subnet);
        self.allocator.free(self.gateway);
        
        if (self.created) {
            self.destroy() catch |err| {
                std.debug.print("[Network] Failed to destroy bridge {s}: {any}\n", .{ self.name, err });
            };
        }
    }
    
    pub fn create(self: *BridgeNetwork) !void {
        std.debug.print("[Network] Creating bridge network: {s}\n", .{self.name});
        
        // Try to create the bridge using ip command
        const create_cmd = try std.fmt.allocPrint(
            self.allocator,
            "ip link add name {s} type bridge",
            .{self.name}
        );
        defer self.allocator.free(create_cmd);
        
        const create_result = try self.runCommand(create_cmd);
        if (create_result != 0) {
            std.debug.print("[Network] Warning: Failed to create bridge (may need root privileges)\n", .{});
        }
        
        // Bring the bridge up
        const up_cmd = try std.fmt.allocPrint(
            self.allocator,
            "ip link set {s} up",
            .{self.name}
        );
        defer self.allocator.free(up_cmd);
        
        const up_result = try self.runCommand(up_cmd);
        if (up_result != 0) {
            std.debug.print("[Network] Warning: Failed to bring bridge up\n", .{});
        }
        
        // Assign IP address to bridge
        const addr_cmd = try std.fmt.allocPrint(
            self.allocator,
            "ip addr add {s} dev {s}",
            .{ self.gateway, self.name }
        );
        defer self.allocator.free(addr_cmd);
        
        const addr_result = try self.runCommand(addr_cmd);
        if (addr_result != 0) {
            std.debug.print("[Network] Warning: Failed to assign IP to bridge\n", .{});
        }
        
        self.created = true;
        std.debug.print("[Network] Bridge network {s} created successfully\n", .{self.name});
    }
    
    pub fn destroy(self: *BridgeNetwork) !void {
        if (!self.created) return;
        
        std.debug.print("[Network] Destroying bridge network: {s}\n", .{self.name});
        
        const destroy_cmd = try std.fmt.allocPrint(
            self.allocator,
            "ip link del {s}",
            .{self.name}
        );
        defer self.allocator.free(destroy_cmd);
        
        const result = try self.runCommand(destroy_cmd);
        if (result != 0) {
            std.debug.print("[Network] Warning: Failed to destroy bridge\n", .{});
        }
        
        self.created = false;
    }
    
    pub fn connectContainer(self: *BridgeNetwork, container_id: []const u8) ![]const u8 {
        // Create veth pair for container
        const veth_host = try std.fmt.allocPrint(
            self.allocator,
            "veth-{s}-host",
            .{container_id[0..8]} // Use first 8 chars of container ID
        );
        
        const veth_container = try std.fmt.allocPrint(
            self.allocator,
            "veth-{s}-cont",
            .{container_id[0..8]}
        );
        
        // Create veth pair
        const create_veth_cmd = try std.fmt.allocPrint(
            self.allocator,
            "ip link add {s} type veth peer name {s}",
            .{ veth_host, veth_container }
        );
        defer self.allocator.free(create_veth_cmd);
        
        const create_result = try self.runCommand(create_veth_cmd);
        if (create_result != 0) {
            std.debug.print("[Network] Warning: Failed to create veth pair\n", .{});
            self.allocator.free(veth_container);
            return veth_host; // Return host side anyway
        }
        
        // Attach host side to bridge
        const attach_cmd = try std.fmt.allocPrint(
            self.allocator,
            "ip link set {s} master {s}",
            .{ veth_host, self.name }
        );
        defer self.allocator.free(attach_cmd);
        
        const attach_result = try self.runCommand(attach_cmd);
        if (attach_result != 0) {
            std.debug.print("[Network] Warning: Failed to attach veth to bridge\n", .{});
        }
        
        // Bring host side up
        const up_host_cmd = try std.fmt.allocPrint(
            self.allocator,
            "ip link set {s} up",
            .{veth_host}
        );
        defer self.allocator.free(up_host_cmd);
        
        _ = try self.runCommand(up_host_cmd);
        
        self.allocator.free(veth_container);
        return veth_host;
    }
    
    fn runCommand(self: *BridgeNetwork, command: []const u8) !u8 {
        // Execute command using child process
        var process = std.process.Child.init(&[_][]const u8{ "/bin/sh", "-c", command }, self.allocator);
        const result = try process.spawnAndWait();
        
        return switch (result) {
            .Exited => |code| @intCast(code),
            else => 1,
        };
    }
};

pub const NetworkManager = struct {
    allocator: std.mem.Allocator,
    networks: std.HashMap([]const u8, *BridgeNetwork, std.hash_map.StringContext, std.hash_map.default_max_load_percentage),
    
    pub fn init(allocator: std.mem.Allocator) NetworkManager {
        return NetworkManager{
            .allocator = allocator,
            .networks = std.HashMap([]const u8, *BridgeNetwork, std.hash_map.StringContext, std.hash_map.default_max_load_percentage).init(allocator),
        };
    }
    
    pub fn deinit(self: *NetworkManager) void {
        var iterator = self.networks.iterator();
        while (iterator.next()) |entry| {
            entry.value_ptr.*.deinit();
            self.allocator.destroy(entry.value_ptr.*);
        }
        self.networks.deinit();
    }
    
    pub fn createNetwork(self: *NetworkManager, config: NetworkConfig) !*BridgeNetwork {
        const network = try self.allocator.create(BridgeNetwork);
        network.* = try BridgeNetwork.init(self.allocator, config);
        
        try network.create();
        try self.networks.put(config.name, network);
        
        return network;
    }
    
    pub fn getNetwork(self: *NetworkManager, name: []const u8) ?*BridgeNetwork {
        return self.networks.get(name);
    }
    
    pub fn destroyNetwork(self: *NetworkManager, name: []const u8) !void {
        if (self.networks.fetchRemove(name)) |entry| {
            entry.value.deinit();
            self.allocator.destroy(entry.value);
        }
    }
};