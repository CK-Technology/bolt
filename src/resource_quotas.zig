const std = @import("std");

pub const QuotaError = error{
    QuotaExceeded,
    InvalidQuotaSpec,
    QuotaNotFound,
    InsufficientPermissions,
};

pub const ResourceType = enum {
    cpu,
    memory,
    storage,
    network_bandwidth,
    capsule_count,
    image_pulls,
    build_time,
    
    pub fn toString(self: ResourceType) []const u8 {
        return switch (self) {
            .cpu => "cpu",
            .memory => "memory", 
            .storage => "storage",
            .network_bandwidth => "network_bandwidth",
            .capsule_count => "capsule_count",
            .image_pulls => "image_pulls",
            .build_time => "build_time",
        };
    }
};

pub const ResourceLimit = struct {
    resource_type: ResourceType,
    limit: u64,
    used: u64 = 0,
    soft_limit: ?u64 = null, // Warning threshold
    
    pub fn isExceeded(self: ResourceLimit) bool {
        return self.used > self.limit;
    }
    
    pub fn isSoftLimitExceeded(self: ResourceLimit) bool {
        if (self.soft_limit) |soft| {
            return self.used > soft;
        }
        return false;
    }
    
    pub fn availableAmount(self: ResourceLimit) u64 {
        if (self.used >= self.limit) {
            return 0;
        }
        return self.limit - self.used;
    }
    
    pub fn utilizationPercent(self: ResourceLimit) f32 {
        if (self.limit == 0) return 0.0;
        return @as(f32, @floatFromInt(self.used)) / @as(f32, @floatFromInt(self.limit)) * 100.0;
    }
};

pub const QuotaScope = enum {
    user,
    namespace,
    cluster,
    node,
};

pub const ResourceQuota = struct {
    name: []const u8,
    scope: QuotaScope,
    scope_identifier: []const u8, // user ID, namespace name, etc.
    limits: std.EnumMap(ResourceType, ResourceLimit),
    created_at: i64,
    updated_at: i64,
    
    pub fn init(allocator: std.mem.Allocator, name: []const u8, scope: QuotaScope, scope_identifier: []const u8) !ResourceQuota {
        return ResourceQuota{
            .name = try allocator.dupe(u8, name),
            .scope = scope,
            .scope_identifier = try allocator.dupe(u8, scope_identifier),
            .limits = std.EnumMap(ResourceType, ResourceLimit).init(.{}),
            .created_at = std.time.timestamp(),
            .updated_at = std.time.timestamp(),
        };
    }
    
    pub fn deinit(self: *ResourceQuota, allocator: std.mem.Allocator) void {
        allocator.free(self.name);
        allocator.free(self.scope_identifier);
    }
    
    pub fn setLimit(self: *ResourceQuota, resource_type: ResourceType, limit: u64, soft_limit: ?u64) void {
        self.limits.put(resource_type, ResourceLimit{
            .resource_type = resource_type,
            .limit = limit,
            .soft_limit = soft_limit,
        });
        self.updated_at = std.time.timestamp();
    }
    
    pub fn checkResourceRequest(self: *ResourceQuota, resource_type: ResourceType, requested: u64) !void {
        if (self.limits.get(resource_type)) |*limit| {
            if (limit.used + requested > limit.limit) {
                std.debug.print("[Quota] Resource request denied: {s} requesting {d} {s}, but quota limit is {d} (used: {d})\n",
                    .{ self.scope_identifier, requested, resource_type.toString(), limit.limit, limit.used });
                return QuotaError.QuotaExceeded;
            }
            
            // Check soft limit warning
            if (limit.soft_limit) |soft| {
                if (limit.used + requested > soft) {
                    std.debug.print("[Quota] Warning: {s} approaching {s} soft limit ({d}/{d})\n",
                        .{ self.scope_identifier, resource_type.toString(), limit.used + requested, soft });
                }
            }
        }
    }
    
    pub fn allocateResource(self: *ResourceQuota, resource_type: ResourceType, amount: u64) !void {
        try self.checkResourceRequest(resource_type, amount);
        
        if (self.limits.getPtr(resource_type)) |limit| {
            limit.used += amount;
            std.debug.print("[Quota] Allocated {d} {s} to {s} (used: {d}/{d})\n",
                .{ amount, resource_type.toString(), self.scope_identifier, limit.used, limit.limit });
        }
    }
    
    pub fn deallocateResource(self: *ResourceQuota, resource_type: ResourceType, amount: u64) void {
        if (self.limits.getPtr(resource_type)) |limit| {
            if (limit.used >= amount) {
                limit.used -= amount;
            } else {
                limit.used = 0;
            }
            std.debug.print("[Quota] Deallocated {d} {s} from {s} (used: {d}/{d})\n",
                .{ amount, resource_type.toString(), self.scope_identifier, limit.used, limit.limit });
        }
    }
    
    pub fn getUsage(self: *const ResourceQuota, resource_type: ResourceType) ?ResourceLimit {
        return self.limits.get(resource_type);
    }
    
    pub fn exportMetrics(self: *const ResourceQuota, allocator: std.mem.Allocator) ![]u8 {
        var metrics = std.ArrayList(u8).init(allocator);
        defer metrics.deinit();
        
        try metrics.appendSlice("# Resource Quota Metrics\n");
        try metrics.appendSlice(try std.fmt.allocPrint(allocator, "quota_name=\"{s}\"\n", .{self.name}));
        const scope_name = @tagName(self.scope);
        try metrics.appendSlice(try std.fmt.allocPrint(allocator, "quota_scope=\"{s}\"\n", .{scope_name}));
        try metrics.appendSlice(try std.fmt.allocPrint(allocator, "quota_identifier=\"{s}\"\n", .{self.scope_identifier}));
        
        var limit_iter = self.limits.iterator();
        while (limit_iter.next()) |entry| {
            const resource_type = entry.key;
            const limit = entry.value;
            
            try metrics.appendSlice(try std.fmt.allocPrint(allocator, 
                "bolt_quota_limit{{resource=\"{s}\",quota=\"{s}\"}} {d}\n",
                .{ resource_type.toString(), self.name, limit.limit }));
            
            try metrics.appendSlice(try std.fmt.allocPrint(allocator, 
                "bolt_quota_used{{resource=\"{s}\",quota=\"{s}\"}} {d}\n",
                .{ resource_type.toString(), self.name, limit.used }));
            
            try metrics.appendSlice(try std.fmt.allocPrint(allocator, 
                "bolt_quota_utilization{{resource=\"{s}\",quota=\"{s}\"}} {d:.2}\n",
                .{ resource_type.toString(), self.name, limit.utilizationPercent() }));
        }
        
        return metrics.toOwnedSlice();
    }
};

pub const QuotaManager = struct {
    allocator: std.mem.Allocator,
    quotas: std.StringHashMap(ResourceQuota),
    scope_quotas: std.StringHashMap(std.StringHashMap([]const u8)), // scope:identifier -> [quota_names]
    
    pub fn init(allocator: std.mem.Allocator) QuotaManager {
        return QuotaManager{
            .allocator = allocator,
            .quotas = std.StringHashMap(ResourceQuota).init(allocator),
            .scope_quotas = std.StringHashMap(std.StringHashMap([]const u8)).init(allocator),
        };
    }
    
    pub fn deinit(self: *QuotaManager) void {
        var quota_iter = self.quotas.iterator();
        while (quota_iter.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            var quota = entry.value_ptr;
            quota.deinit(self.allocator);
        }
        self.quotas.deinit();
        
        var scope_iter = self.scope_quotas.iterator();
        while (scope_iter.next()) |entry| {
            self.allocator.free(entry.key_ptr.*);
            var scope_map = entry.value_ptr;
            
            var name_iter = scope_map.iterator();
            while (name_iter.next()) |name_entry| {
                self.allocator.free(name_entry.key_ptr.*);
                self.allocator.free(name_entry.value_ptr.*);
            }
            scope_map.deinit();
        }
        self.scope_quotas.deinit();
    }
    
    pub fn createQuota(self: *QuotaManager, name: []const u8, scope: QuotaScope, scope_identifier: []const u8) !void {
        // Check if quota already exists
        if (self.quotas.contains(name)) {
            std.debug.print("[QuotaManager] Quota {s} already exists\n", .{name});
            return QuotaError.InvalidQuotaSpec;
        }
        
        const quota = try ResourceQuota.init(self.allocator, name, scope, scope_identifier);
        const quota_name = try self.allocator.dupe(u8, name);
        try self.quotas.put(quota_name, quota);
        
        // Track quota by scope
        const scope_name = @tagName(scope);
        const scope_key = try std.fmt.allocPrint(self.allocator, "{s}:{s}", .{ scope_name, scope_identifier });
        
        var scope_quota_map = self.scope_quotas.get(scope_key) orelse blk: {
            const new_map = std.StringHashMap([]const u8).init(self.allocator);
            try self.scope_quotas.put(scope_key, new_map);
            break :blk self.scope_quotas.get(scope_key).?;
        };
        
        const quota_name_copy = try self.allocator.dupe(u8, name);
        try scope_quota_map.put(quota_name_copy, quota_name_copy);
        
        const scope_name_debug = @tagName(scope);
        std.debug.print("[QuotaManager] Created quota: {s} for {s}:{s}\n", .{ name, scope_name_debug, scope_identifier });
    }
    
    pub fn setQuotaLimit(self: *QuotaManager, quota_name: []const u8, resource_type: ResourceType, limit: u64, soft_limit: ?u64) !void {
        if (self.quotas.getPtr(quota_name)) |quota| {
            quota.setLimit(resource_type, limit, soft_limit);
            std.debug.print("[QuotaManager] Set {s} limit for quota {s}: {d} (soft: {?d})\n", 
                .{ resource_type.toString(), quota_name, limit, soft_limit });
        } else {
            return QuotaError.QuotaNotFound;
        }
    }
    
    pub fn checkResourceRequest(self: *QuotaManager, scope: QuotaScope, scope_identifier: []const u8, 
                               resource_type: ResourceType, requested: u64) !void {
        const quotas = try self.getQuotasForScope(scope, scope_identifier);
        
        for (quotas) |quota_name| {
            if (self.quotas.getPtr(quota_name)) |quota| {
                try quota.checkResourceRequest(resource_type, requested);
            }
        }
    }
    
    pub fn allocateResource(self: *QuotaManager, scope: QuotaScope, scope_identifier: []const u8,
                           resource_type: ResourceType, amount: u64) !void {
        const quotas = try self.getQuotasForScope(scope, scope_identifier);
        
        // Check all quotas first
        for (quotas) |quota_name| {
            if (self.quotas.getPtr(quota_name)) |quota| {
                try quota.checkResourceRequest(resource_type, amount);
            }
        }
        
        // If all checks pass, allocate on all quotas
        for (quotas) |quota_name| {
            if (self.quotas.getPtr(quota_name)) |quota| {
                try quota.allocateResource(resource_type, amount);
            }
        }
    }
    
    pub fn deallocateResource(self: *QuotaManager, scope: QuotaScope, scope_identifier: []const u8,
                             resource_type: ResourceType, amount: u64) void {
        const quotas = self.getQuotasForScope(scope, scope_identifier) catch return;
        
        for (quotas) |quota_name| {
            if (self.quotas.getPtr(quota_name)) |quota| {
                quota.deallocateResource(resource_type, amount);
            }
        }
    }
    
    pub fn getQuotaUsage(self: *QuotaManager, quota_name: []const u8) !std.EnumMap(ResourceType, ResourceLimit) {
        if (self.quotas.get(quota_name)) |quota| {
            return quota.limits;
        }
        return QuotaError.QuotaNotFound;
    }
    
    pub fn createDefaultQuotas(self: *QuotaManager) !void {
        std.debug.print("[QuotaManager] Creating default quotas\n");
        
        // Default cluster-wide quota
        try self.createQuota("cluster-default", .cluster, "default");
        try self.setQuotaLimit("cluster-default", .cpu, 1000, 800);           // 1000 cores (soft: 800)
        try self.setQuotaLimit("cluster-default", .memory, 2048, 1500);       // 2TB RAM (soft: 1.5TB)
        try self.setQuotaLimit("cluster-default", .storage, 10240, 8192);     // 10TB storage (soft: 8TB)
        try self.setQuotaLimit("cluster-default", .capsule_count, 10000, 8000); // 10k capsules (soft: 8k)
        try self.setQuotaLimit("cluster-default", .image_pulls, 100000, 80000); // 100k pulls/day (soft: 80k)
        
        // Default user quota
        try self.createQuota("user-default", .user, "default");
        try self.setQuotaLimit("user-default", .cpu, 16, 12);                 // 16 cores (soft: 12)
        try self.setQuotaLimit("user-default", .memory, 64, 48);              // 64GB RAM (soft: 48GB)
        try self.setQuotaLimit("user-default", .storage, 500, 400);           // 500GB storage (soft: 400GB)
        try self.setQuotaLimit("user-default", .capsule_count, 100, 80);      // 100 capsules (soft: 80)
        try self.setQuotaLimit("user-default", .image_pulls, 1000, 800);      // 1k pulls/day (soft: 800)
        try self.setQuotaLimit("user-default", .build_time, 3600, 2400);      // 1 hour build time (soft: 40min)
        
        // Default namespace quota
        try self.createQuota("namespace-default", .namespace, "default");
        try self.setQuotaLimit("namespace-default", .cpu, 64, 48);            // 64 cores (soft: 48)
        try self.setQuotaLimit("namespace-default", .memory, 256, 192);       // 256GB RAM (soft: 192GB)
        try self.setQuotaLimit("namespace-default", .storage, 2048, 1500);    // 2TB storage (soft: 1.5TB)
        try self.setQuotaLimit("namespace-default", .capsule_count, 500, 400); // 500 capsules (soft: 400)
        
        std.debug.print("[QuotaManager] Default quotas created successfully\n");
    }
    
    pub fn generateQuotaReport(self: *QuotaManager, allocator: std.mem.Allocator) ![]u8 {
        var report = std.ArrayList(u8).init(allocator);
        defer report.deinit();
        
        try report.appendSlice("# Bolt Resource Quota Report\n");
        try report.appendSlice(try std.fmt.allocPrint(allocator, "Generated at: {d}\n\n", .{std.time.timestamp()}));
        
        var quota_iter = self.quotas.iterator();
        while (quota_iter.next()) |entry| {
            const quota_name = entry.key_ptr.*;
            const quota = entry.value_ptr;
            
            try report.appendSlice(try std.fmt.allocPrint(allocator, "## Quota: {s}\n", .{quota_name}));
            const scope_name_report = @tagName(quota.scope);
            try report.appendSlice(try std.fmt.allocPrint(allocator, "- Scope: {s}:{s}\n", .{ scope_name_report, quota.scope_identifier }));
            try report.appendSlice(try std.fmt.allocPrint(allocator, "- Created: {d}\n", .{quota.created_at}));
            try report.appendSlice(try std.fmt.allocPrint(allocator, "- Updated: {d}\n\n", .{quota.updated_at}));
            
            try report.appendSlice("### Resource Usage:\n");
            var limit_iter = quota.limits.iterator();
            while (limit_iter.next()) |limit_entry| {
                const resource_type = limit_entry.key;
                const limit = limit_entry.value;
                
                const status = if (limit.isExceeded()) "EXCEEDED" else if (limit.isSoftLimitExceeded()) "WARNING" else "OK";
                
                try report.appendSlice(try std.fmt.allocPrint(allocator, 
                    "- {s}: {d}/{d} ({d:.1}%) [{s}]\n",
                    .{ resource_type.toString(), limit.used, limit.limit, limit.utilizationPercent(), status }));
            }
            try report.appendSlice("\n");
        }
        
        return report.toOwnedSlice();
    }
    
    // Private helper methods
    fn getQuotasForScope(self: *QuotaManager, scope: QuotaScope, scope_identifier: []const u8) ![][]const u8 {
        const scope_name_helper = @tagName(scope);
        const scope_key = try std.fmt.allocPrint(self.allocator, "{s}:{s}", .{ scope_name_helper, scope_identifier });
        defer self.allocator.free(scope_key);
        
        if (self.scope_quotas.get(scope_key)) |scope_map| {
            var quotas = std.ArrayList([]const u8).init(self.allocator);
            defer quotas.deinit();
            
            var name_iter = scope_map.iterator();
            while (name_iter.next()) |entry| {
                try quotas.append(entry.value_ptr.*);
            }
            
            return quotas.toOwnedSlice();
        }
        
        return &[_][]const u8{};
    }
};