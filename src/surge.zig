const std = @import("std");
const oci = @import("oci.zig");
const capsule = @import("capsule.zig");
const network = @import("network.zig");
const quic_fabric = @import("quic_fabric.zig");
const bolt_dns = @import("bolt_dns.zig");
const ContentStore = @import("content_store.zig");

pub const SurgeError = error{
    BoltfileNotFound,
    InvalidBoltfile,
    ServiceStartFailed,
    ServiceStopFailed,
    DependencyError,
};

pub const ServiceConfig = struct {
    name: []const u8,
    image: ?[]const u8 = null,
    build: ?[]const u8 = null,
    capsule: ?[]const u8 = null,
    ports: [][]const u8 = &.{},
    volumes: [][]const u8 = &.{},
    environment: std.StringHashMap([]const u8),
    depends_on: [][]const u8 = &.{},
    networks: [][]const u8 = &.{},
    
    pub fn deinit(self: *ServiceConfig, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
        if (self.image) |image| allocator.free(image);
        if (self.build) |build| allocator.free(build);
        if (self.capsule) |cap| allocator.free(cap);
        
        for (self.ports) |port| {
            allocator.free(port);
        }
        allocator.free(self.ports);
        
        for (self.volumes) |volume| {
            allocator.free(volume);
        }
        allocator.free(self.volumes);
        
        var env_iterator = self.environment.iterator();
        while (env_iterator.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.*);
        }
        self.environment.deinit();
        
        for (self.depends_on) |dep| {
            allocator.free(dep);
        }
        allocator.free(self.depends_on);
        
        for (self.networks) |net| {
            allocator.free(net);
        }
        allocator.free(self.networks);
    }
};

pub const BoltfileConfig = struct {
    project: []const u8,
    services: std.StringHashMap(ServiceConfig),
    networks: std.StringHashMap(network.NetworkConfig),
    volumes: std.StringHashMap(VolumeConfig),
    quic_fabric: ?QuicFabricConfig = null,
    dns_config: ?DNSConfig = null,
    
    pub const VolumeConfig = struct {
        name: []const u8,
        driver: []const u8 = "local",
        size: ?[]const u8 = null,
    };
    
    pub const QuicFabricConfig = struct {
        enabled: bool = true,
        node_id: ?[]const u8 = null,
        bind_address: []const u8 = "127.0.0.1",
        bind_port: u16 = 4433, // Default QUIC port
        encryption: bool = true,
        service_discovery: bool = true,
        
        pub fn deinit(self: *QuicFabricConfig, allocator: std.mem.Allocator) void {
            if (self.node_id) |id| allocator.free(id);
            allocator.free(self.bind_address);
        }
    };
    
    pub const DNSConfig = struct {
        enabled: bool = true,
        port: u16 = 5353, // mDNS port
        domain: []const u8 = "bolt.local",
        
        pub fn deinit(self: *DNSConfig, allocator: std.mem.Allocator) void {
            allocator.free(self.domain);
        }
    };
    
    pub fn deinit(self: *BoltfileConfig, allocator: std.mem.Allocator) void {
        allocator.free(self.project);
        
        var service_iterator = self.services.iterator();
        while (service_iterator.next()) |entry| {
            var service = entry.value_ptr;
            service.deinit(allocator);
        }
        self.services.deinit();
        
        var network_iterator = self.networks.iterator();
        while (network_iterator.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.name);
            if (entry.value_ptr.subnet) |subnet| allocator.free(subnet);
            if (entry.value_ptr.gateway) |gateway| allocator.free(gateway);
            for (entry.value_ptr.dns_servers) |dns| {
                allocator.free(dns);
            }
            allocator.free(entry.value_ptr.dns_servers);
        }
        self.networks.deinit();
        
        var volume_iterator = self.volumes.iterator();
        while (volume_iterator.next()) |entry| {
            allocator.free(entry.key_ptr.*);
            allocator.free(entry.value_ptr.name);
            allocator.free(entry.value_ptr.driver);
            if (entry.value_ptr.size) |size| allocator.free(size);
        }
        self.volumes.deinit();
        
        if (self.quic_fabric) |*fabric_config| {
            fabric_config.deinit(allocator);
        }
        
        if (self.dns_config) |*dns_config| {
            dns_config.deinit(allocator);
        }
    }
};

pub const RunningService = struct {
    name: []const u8,
    capsule_instance: *capsule.Capsule,
    pid: ?std.process.Child.Id = null,
    status: ServiceStatus = .starting,
    
    pub const ServiceStatus = enum {
        starting,
        running,
        stopping,
        stopped,
        failed,
    };
};

pub const Orchestrator = struct {
    allocator: std.mem.Allocator,
    network_manager: network.NetworkManager,
    running_services: std.StringHashMap(*RunningService),
    quic_fabric: ?quic_fabric.QuicFabric = null,
    dns_server: ?bolt_dns.BoltDNSServer = null,
    service_discovery: ?bolt_dns.ServiceDiscovery = null,
    
    pub fn init(allocator: std.mem.Allocator) !Orchestrator {
        return Orchestrator{
            .allocator = allocator,
            .network_manager = network.NetworkManager.init(allocator),
            .running_services = std.StringHashMap(*RunningService).init(allocator),
        };
    }
    
    pub fn deinit(self: *Orchestrator) void {
        var service_iterator = self.running_services.iterator();
        while (service_iterator.next()) |entry| {
            entry.value_ptr.*.capsule_instance.deinit();
            self.allocator.destroy(entry.value_ptr.*.capsule_instance);
            self.allocator.free(entry.value_ptr.*.name);
            self.allocator.destroy(entry.value_ptr.*);
        }
        self.running_services.deinit();
        
        self.network_manager.deinit();
        
        if (self.quic_fabric) |*fabric| {
            fabric.deinit();
        }
        
        if (self.dns_server) |*dns| {
            dns.deinit();
        }
        
        if (self.service_discovery) |*discovery| {
            _ = discovery; // ServiceDiscovery doesn't have deinit currently
        }
    }
    
    pub fn up(self: *Orchestrator, boltfile_path: []const u8) !void {
        std.debug.print("[Surge] Reading Boltfile: {s}\n", .{boltfile_path});
        
        // Parse Boltfile
        const boltfile_content = try self.readBoltfile(boltfile_path);
        defer self.allocator.free(boltfile_content);
        
        var config = try self.parseBoltfile(boltfile_content);
        defer config.deinit(self.allocator);
        
        std.debug.print("[Surge] Starting project: {s}\n", .{config.project});
        
        // Initialize advanced networking if configured
        try self.initializeAdvancedNetworking(&config);
        
        // Create networks first
        try self.createNetworks(&config);
        
        // Start services in dependency order
        try self.startServices(&config);
        
        std.debug.print("[Surge] All services started successfully\n", .{});
    }
    
    pub fn down(self: *Orchestrator, boltfile_path: []const u8) !void {
        std.debug.print("[Surge] Stopping services from: {s}\n", .{boltfile_path});
        
        // For now, stop all running services
        var service_iterator = self.running_services.iterator();
        while (service_iterator.next()) |entry| {
            const service = entry.value_ptr.*;
            std.debug.print("[Surge] Stopping service: {s}\n", .{service.name});
            
            if (service.pid) |pid| {
                // Send SIGTERM
                std.posix.kill(pid, std.posix.SIG.TERM) catch |err| {
                    std.debug.print("[Surge] Failed to stop service {s}: {any}\n", .{ service.name, err });
                };
            }
        }
        
        std.debug.print("[Surge] All services stopped\n", .{});
    }
    
    pub fn kill(self: *Orchestrator, boltfile_path: []const u8) !void {
        std.debug.print("[Surge] Force killing services from: {s}\n", .{boltfile_path});
        
        // Force kill all running services
        var service_iterator = self.running_services.iterator();
        while (service_iterator.next()) |entry| {
            const service = entry.value_ptr.*;
            std.debug.print("[Surge] Force killing service: {s}\n", .{service.name});
            
            if (service.pid) |pid| {
                // Send SIGKILL
                std.posix.kill(pid, std.posix.SIG.KILL) catch |err| {
                    std.debug.print("[Surge] Failed to kill service {s}: {any}\n", .{ service.name, err });
                };
            }
        }
        
        std.debug.print("[Surge] All services force killed\n", .{});
    }
    
    fn readBoltfile(self: *Orchestrator, path: []const u8) ![]u8 {
        const file = std.fs.cwd().openFile(path, .{}) catch |err| switch (err) {
            error.FileNotFound => {
                std.debug.print("[Surge] Boltfile not found: {s}\n", .{path});
                return SurgeError.BoltfileNotFound;
            },
            else => return err,
        };
        defer file.close();
        
        const file_size = try file.getEndPos();
        const content = try self.allocator.alloc(u8, file_size);
        _ = try file.readAll(content);
        
        return content;
    }
    
    fn parseBoltfile(self: *Orchestrator, content: []const u8) !BoltfileConfig {
        // Simple TOML-like parser for basic Boltfile support
        // This is a very basic implementation - a real parser would be more robust
        
        var config = BoltfileConfig{
            .project = try self.allocator.dupe(u8, "default"),
            .services = std.StringHashMap(ServiceConfig).init(self.allocator),
            .networks = std.StringHashMap(network.NetworkConfig).init(self.allocator),
            .volumes = std.StringHashMap(BoltfileConfig.VolumeConfig).init(self.allocator),
        };
        
        var lines = std.mem.splitSequence(u8, content, "\n");
        var current_service: ?[]const u8 = null;
        var current_service_config: ?ServiceConfig = null;
        
        while (lines.next()) |line| {
            const trimmed = std.mem.trim(u8, line, " \t\r\n");
            if (trimmed.len == 0 or trimmed[0] == '#') continue;
            
            if (std.mem.startsWith(u8, trimmed, "project = ")) {
                const project_start = std.mem.indexOf(u8, trimmed, "\"") orelse continue;
                const project_end = std.mem.lastIndexOf(u8, trimmed, "\"") orelse continue;
                if (project_start != project_end) {
                    self.allocator.free(config.project);
                    config.project = try self.allocator.dupe(u8, trimmed[project_start + 1..project_end]);
                }
            } else if (std.mem.startsWith(u8, trimmed, "[services.")) {
                // Save previous service if exists
                if (current_service != null and current_service_config != null) {
                    try config.services.put(current_service.?, current_service_config.?);
                }
                
                // Extract service name
                const start = "[services.".len;
                const end = std.mem.indexOf(u8, trimmed[start..], "]") orelse continue;
                current_service = try self.allocator.dupe(u8, trimmed[start..start + end]);
                current_service_config = ServiceConfig{
                    .name = try self.allocator.dupe(u8, current_service.?),
                    .environment = std.StringHashMap([]const u8).init(self.allocator),
                };
            } else if (current_service_config != null) {
                // Parse service properties
                if (std.mem.startsWith(u8, trimmed, "image = ")) {
                    const value = try self.extractStringValue(trimmed);
                    current_service_config.?.image = try self.allocator.dupe(u8, value);
                } else if (std.mem.startsWith(u8, trimmed, "capsule = ")) {
                    const value = try self.extractStringValue(trimmed);
                    current_service_config.?.capsule = try self.allocator.dupe(u8, value);
                }
            }
        }
        
        // Save last service
        if (current_service != null and current_service_config != null) {
            try config.services.put(current_service.?, current_service_config.?);
        }
        
        return config;
    }
    
    fn extractStringValue(self: *Orchestrator, line: []const u8) ![]const u8 {
        _ = self;
        const start = std.mem.indexOf(u8, line, "\"") orelse return "";
        const end = std.mem.lastIndexOf(u8, line, "\"") orelse return "";
        if (start == end) return "";
        return line[start + 1..end];
    }
    
    fn createNetworks(self: *Orchestrator, config: *BoltfileConfig) !void {
        // Create default network if none specified
        if (config.networks.count() == 0) {
            const default_network = network.NetworkConfig{
                .name = "bolt-default",
                .network_type = .bridge,
                .subnet = "172.17.0.0/16",
                .gateway = "172.17.0.1",
            };
            
            _ = try self.network_manager.createNetwork(default_network);
        }
        
        var network_iterator = config.networks.iterator();
        while (network_iterator.next()) |entry| {
            _ = try self.network_manager.createNetwork(entry.value_ptr.*);
        }
    }
    
    fn startServices(self: *Orchestrator, config: *BoltfileConfig) !void {
        var service_iterator = config.services.iterator();
        while (service_iterator.next()) |entry| {
            const service_name = entry.key_ptr.*;
            const service_config = entry.value_ptr.*;
            
            std.debug.print("[Surge] Starting service: {s}\n", .{service_name});
            
                // Get or pull image
            var image: oci.ImageManifest = undefined;
            var should_free_image = false;
            
            if (service_config.image) |image_name| {
                image = oci.findLocalImage(self.allocator, image_name) catch blk: {
                    std.debug.print("[Surge] Pulling image for service {s}: {s}\n", .{ service_name, image_name });
                    // Initialize content store for deduplication
                    var store = try ContentStore.ContentAddressedStore.init(self.allocator, ".bolt/cas");
                    defer store.deinit();
                    var puller = try oci.ImagePuller.init(self.allocator, &store);
                    defer puller.deinit();
                    try puller.pull(image_name);
                    break :blk try oci.findLocalImage(self.allocator, image_name);
                };
                should_free_image = true;
            } else {
                // Create mock image for capsule-based services
                image = oci.ImageManifest{
                    .name = try self.allocator.dupe(u8, service_config.capsule orelse "default"),
                    .tag = try self.allocator.dupe(u8, "latest"),
                    .digest = try self.allocator.dupe(u8, "sha256:mock"),
                    .layers = try self.allocator.alloc(oci.Layer, 0),
                    .config = oci.ImageConfig{
                        .entrypoint = null,
                        .cmd = null,
                        .env = try self.allocator.alloc([]const u8, 0),
                        .working_dir = try self.allocator.dupe(u8, "/"),
                        .user = try self.allocator.dupe(u8, "root"),
                    },
                };
                should_free_image = true;
            }
            defer if (should_free_image) {
                var mutable_image = image;
                mutable_image.deinit(self.allocator);
            };
            
            // Create capsule
            const service_capsule = try self.allocator.create(capsule.Capsule);
            service_capsule.* = try capsule.Capsule.init(self.allocator, image);
            
            // Create running service record
            const running_service = try self.allocator.create(RunningService);
            running_service.* = RunningService{
                .name = try self.allocator.dupe(u8, service_name),
                .capsule_instance = service_capsule,
            };
            
            try self.running_services.put(service_name, running_service);
            
            // Register service in DNS and QUIC fabric
            if (self.service_discovery) |*discovery| {
                try discovery.announceService(service_name, 8080); // Default service port
            }
            
            // Start the service (in background for now)
            std.debug.print("[Surge] Service {s} configuration complete\n", .{service_name});
        }
    }
    
    fn initializeAdvancedNetworking(self: *Orchestrator, config: *BoltfileConfig) !void {
        // Initialize QUIC fabric if enabled
        if (config.quic_fabric) |fabric_config| {
            if (fabric_config.enabled) {
                const node_id = fabric_config.node_id orelse try std.fmt.allocPrint(self.allocator, "bolt-node-{d}", .{std.time.timestamp()});
                defer if (fabric_config.node_id == null) self.allocator.free(node_id);
                
                self.quic_fabric = try quic_fabric.QuicFabric.init(
                    self.allocator,
                    node_id,
                    fabric_config.bind_address,
                    fabric_config.bind_port
                );
                
                try self.quic_fabric.?.startFabric();
                std.debug.print("[Surge] QUIC fabric initialized\n", .{});
            }
        } else {
            // Default QUIC fabric setup
            const default_node_id = try std.fmt.allocPrint(self.allocator, "bolt-node-{d}", .{std.time.timestamp()});
            defer self.allocator.free(default_node_id);
            
            self.quic_fabric = try quic_fabric.QuicFabric.init(
                self.allocator,
                default_node_id,
                "127.0.0.1",
                4433
            );
            
            try self.quic_fabric.?.startFabric();
            std.debug.print("[Surge] Default QUIC fabric initialized\n", .{});
        }
        
        // Initialize DNS server if enabled
        if (config.dns_config) |dns_config| {
            if (dns_config.enabled) {
                self.dns_server = bolt_dns.BoltDNSServer.init(self.allocator);
                self.dns_server.?.bind_port = dns_config.port;
                
                try self.dns_server.?.start();
                std.debug.print("[Surge] DNS server initialized on port {d}\n", .{dns_config.port});
            }
        } else {
            // Default DNS setup
            self.dns_server = bolt_dns.BoltDNSServer.init(self.allocator);
            try self.dns_server.?.start();
            std.debug.print("[Surge] Default DNS server initialized\n", .{});
        }
        
        // Initialize service discovery
        if (self.dns_server) |*dns| {
            self.service_discovery = bolt_dns.ServiceDiscovery.init(self.allocator, dns);
            if (self.quic_fabric) |*fabric| {
                self.service_discovery.?.setQuicFabric(fabric);
            }
            std.debug.print("[Surge] Service discovery initialized\n", .{});
        }
    }
};