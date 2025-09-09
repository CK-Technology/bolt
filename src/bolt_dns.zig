const std = @import("std");
const quic_fabric = @import("quic_fabric.zig");

pub const DNSRecord = struct {
    name: []const u8,
    record_type: DNSRecordType,
    target: []const u8,
    port: u16,
    ttl: u32 = 300, // 5 minutes default TTL
    
    pub const DNSRecordType = enum {
        A,      // IPv4 address
        AAAA,   // IPv6 address  
        SRV,    // Service record
        TXT,    // Text record
        CNAME,  // Canonical name
    };
    
    pub fn deinit(self: *DNSRecord, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
        allocator.free(self.target);
    }
};

pub const BoltDNSServer = struct {
    allocator: std.mem.Allocator,
    records: std.StringHashMap(DNSRecord),
    fabric: ?*quic_fabric.QuicFabric = null,
    bind_port: u16 = 5353, // mDNS port
    
    pub fn init(allocator: std.mem.Allocator) BoltDNSServer {
        return BoltDNSServer{
            .allocator = allocator,
            .records = std.StringHashMap(DNSRecord).init(allocator),
        };
    }
    
    pub fn deinit(self: *BoltDNSServer) void {
        var record_iterator = self.records.iterator();
        while (record_iterator.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            var record = entry.value_ptr;
            record.deinit(self.allocator);
        }
        self.records.deinit();
    }
    
    pub fn setQuicFabric(self: *BoltDNSServer, fabric: *quic_fabric.QuicFabric) void {
        self.fabric = fabric;
        std.debug.print("[BoltDNS] Connected to QUIC fabric\n", .{});
    }
    
    pub fn start(self: *BoltDNSServer) !void {
        std.debug.print("[BoltDNS] Starting DNS server on port {d}\n", .{self.bind_port});
        
        // Initialize DNS server
        try self.startDNSListener();
        
        // Register default records
        try self.registerDefaultRecords();
        
        std.debug.print("[BoltDNS] DNS server started successfully\n", .{});
    }
    
    pub fn registerServiceRecord(self: *BoltDNSServer, service_name: []const u8, address: []const u8, port: u16) !void {
        // Create SRV record for the service
        const srv_record = DNSRecord{
            .name = try self.allocator.dupe(u8, service_name),
            .record_type = .SRV,
            .target = try self.allocator.dupe(u8, address),
            .port = port,
            .ttl = 300,
        };
        
        const record_name = try std.fmt.allocPrint(self.allocator, "_bolt._tcp.{s}.local", .{service_name});
        defer self.allocator.free(record_name);
        
        const stored_name = try self.allocator.dupe(u8, record_name);
        try self.records.put(stored_name, srv_record);
        
        // Also create A record for direct IP resolution
        const a_record = DNSRecord{
            .name = try self.allocator.dupe(u8, service_name),
            .record_type = .A,
            .target = try self.allocator.dupe(u8, address),
            .port = port,
            .ttl = 300,
        };
        
        const a_record_name = try std.fmt.allocPrint(self.allocator, "{s}.bolt.local", .{service_name});
        defer self.allocator.free(a_record_name);
        
        const stored_a_name = try self.allocator.dupe(u8, a_record_name);
        try self.records.put(stored_a_name, a_record);
        
        std.debug.print("[BoltDNS] Registered service: {s} -> {s}:{d}\n", .{ service_name, address, port });
        
        // Propagate to QUIC fabric if available
        if (self.fabric) |fabric| {
            try fabric.registerLocalService(service_name, port);
        }
    }
    
    pub fn resolveService(self: *BoltDNSServer, query_name: []const u8) !?DNSRecord {
        std.debug.print("[BoltDNS] Resolving query: {s}\n", .{query_name});
        
        // Check local records first
        if (self.records.get(query_name)) |record| {
            std.debug.print("[BoltDNS] Found local record for: {s}\n", .{query_name});
            return record;
        }
        
        // Try different naming patterns
        const patterns = [_][]const u8{
            "{s}.bolt.local",
            "_bolt._tcp.{s}.local",
            "{s}",
        };
        
        for (patterns) |pattern| {
            const formatted_name = try std.fmt.allocPrint(self.allocator, pattern, .{query_name});
            defer self.allocator.free(formatted_name);
            
            if (self.records.get(formatted_name)) |record| {
                std.debug.print("[BoltDNS] Found record with pattern {s}: {s}\n", .{ pattern, formatted_name });
                return record;
            }
        }
        
        // If not found locally, query QUIC fabric
        if (self.fabric) |fabric| {
            const service = fabric.connectToService(query_name) catch |err| switch (err) {
                error.ServiceDiscoveryFailed => return null,
                else => return err,
            };
            
            // Convert fabric service to DNS record
            return DNSRecord{
                .name = try self.allocator.dupe(u8, service.name),
                .record_type = .SRV,
                .target = try self.allocator.dupe(u8, service.address),
                .port = service.port,
                .ttl = 60, // Shorter TTL for remote services
            };
        }
        
        std.debug.print("[BoltDNS] No record found for: {s}\n", .{query_name});
        return null;
    }
    
    pub fn createServiceAlias(self: *BoltDNSServer, alias: []const u8, target_service: []const u8) !void {
        const target_record_name = try std.fmt.allocPrint(self.allocator, "{s}.bolt.local", .{target_service});
        defer self.allocator.free(target_record_name);
        
        const cname_record = DNSRecord{
            .name = try self.allocator.dupe(u8, alias),
            .record_type = .CNAME,
            .target = try self.allocator.dupe(u8, target_record_name),
            .port = 0,
            .ttl = 300,
        };
        
        const alias_name = try std.fmt.allocPrint(self.allocator, "{s}.bolt.local", .{alias});
        defer self.allocator.free(alias_name);
        
        const stored_alias = try self.allocator.dupe(u8, alias_name);
        try self.records.put(stored_alias, cname_record);
        
        std.debug.print("[BoltDNS] Created alias: {s} -> {s}\n", .{ alias, target_service });
    }
    
    fn startDNSListener(self: *BoltDNSServer) !void {
        // This would start the actual DNS server
        // For now, simulate DNS server startup
        
        std.debug.print("[BoltDNS] DNS listener initialized\n", .{});
        
        _ = self; // Avoid unused variable warning for now
        
        // In a full implementation, this would:
        // 1. Bind to UDP port 5353 (mDNS)
        // 2. Handle DNS queries
        // 3. Respond with appropriate records
        // 4. Support multicast DNS for service discovery
    }
    
    fn registerDefaultRecords(self: *BoltDNSServer) !void {
        // Register localhost record
        const localhost_record = DNSRecord{
            .name = try self.allocator.dupe(u8, "localhost"),
            .record_type = .A,
            .target = try self.allocator.dupe(u8, "127.0.0.1"),
            .port = 0,
            .ttl = 86400, // 24 hours
        };
        
        const localhost_name = try self.allocator.dupe(u8, "localhost.bolt.local");
        try self.records.put(localhost_name, localhost_record);
        
        // Register bolt-dns service itself
        const dns_record = DNSRecord{
            .name = try self.allocator.dupe(u8, "bolt-dns"),
            .record_type = .SRV,
            .target = try self.allocator.dupe(u8, "127.0.0.1"),
            .port = self.bind_port,
            .ttl = 300,
        };
        
        const dns_name = try self.allocator.dupe(u8, "_bolt._tcp.bolt-dns.local");
        try self.records.put(dns_name, dns_record);
        
        std.debug.print("[BoltDNS] Default records registered\n", .{});
    }
};

pub const ServiceDiscovery = struct {
    allocator: std.mem.Allocator,
    dns_server: *BoltDNSServer,
    fabric: ?*quic_fabric.QuicFabric = null,
    
    pub fn init(allocator: std.mem.Allocator, dns_server: *BoltDNSServer) ServiceDiscovery {
        return ServiceDiscovery{
            .allocator = allocator,
            .dns_server = dns_server,
        };
    }
    
    pub fn setQuicFabric(self: *ServiceDiscovery, fabric: *quic_fabric.QuicFabric) void {
        self.fabric = fabric;
        self.dns_server.setQuicFabric(fabric);
    }
    
    pub fn announceService(self: *ServiceDiscovery, service_name: []const u8, port: u16) !void {
        const local_address = "127.0.0.1"; // For local development
        
        // Register in DNS
        try self.dns_server.registerServiceRecord(service_name, local_address, port);
        
        // Announce via QUIC fabric if available
        if (self.fabric) |fabric| {
            try fabric.registerLocalService(service_name, port);
        }
        
        std.debug.print("[ServiceDiscovery] Service announced: {s}:{d}\n", .{ service_name, port });
    }
    
    pub fn discoverService(self: *ServiceDiscovery, service_name: []const u8) !?quic_fabric.ServiceEndpoint {
        // Try DNS resolution first
        if (try self.dns_server.resolveService(service_name)) |record| {
            return quic_fabric.ServiceEndpoint{
                .name = try self.allocator.dupe(u8, record.name),
                .address = try self.allocator.dupe(u8, record.target),
                .port = record.port,
                .protocol = try self.allocator.dupe(u8, "quic"),
                .encryption_key = null, // Will be resolved via fabric
            };
        }
        
        // Fallback to QUIC fabric discovery
        if (self.fabric) |fabric| {
            return fabric.connectToService(service_name) catch null;
        }
        
        return null;
    }
};