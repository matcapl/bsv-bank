// core/common/src/metrics.rs
// Prometheus metrics collection

use prometheus::{
    Counter, Histogram, HistogramOpts, HistogramVec,
    // CounterVec, Gauge, GaugeVec, 
    IntCounter, IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};
use std::time::Instant;

/// Service-wide metrics
#[derive(Clone)]
pub struct ServiceMetrics {
    // HTTP metrics
    pub http_request_counter: Counter,
    pub http_request_duration: Histogram,
    pub http_requests_total: IntCounterVec,
    pub http_request_duration_seconds: HistogramVec,
    pub http_requests_in_progress: IntGaugeVec,
    
    // Error metrics
    pub errors_total: IntCounterVec,
    
    // Business metrics
    pub business_operations_total: IntCounterVec,
    pub business_operation_duration_seconds: HistogramVec,
}

impl ServiceMetrics {
    pub fn new(registry: &Registry, service_name: &str) -> Result<Self, prometheus::Error> {
        // Create basic counter for backward compatibility
        let http_request_counter = Counter::new(
            format!("{}_http_requests", service_name),
            "Total HTTP requests (simple counter)"
        )?;
        registry.register(Box::new(http_request_counter.clone()))?;
        
        // Create basic histogram for backward compatibility
        let http_request_duration = Histogram::with_opts(
            HistogramOpts::new(
                format!("{}_http_request_duration", service_name),
                "HTTP request duration (simple histogram)"
            )
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0])
        )?;
        registry.register(Box::new(http_request_duration.clone()))?;
        
        let http_requests_total = IntCounterVec::new(
            Opts::new("http_requests_total", "Total number of HTTP requests")
                .namespace(service_name),
            &["method", "endpoint", "status"],
        )?;
        registry.register(Box::new(http_requests_total.clone()))?;
        
        let http_request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "http_request_duration_seconds",
                "HTTP request duration in seconds",
            )
            .namespace(service_name)
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
            &["method", "endpoint"],
        )?;
        registry.register(Box::new(http_request_duration_seconds.clone()))?;
        
        let http_requests_in_progress = IntGaugeVec::new(
            Opts::new("http_requests_in_progress", "Number of HTTP requests currently being processed")
                .namespace(service_name),
            &["method", "endpoint"],
        )?;
        registry.register(Box::new(http_requests_in_progress.clone()))?;
        
        let errors_total = IntCounterVec::new(
            Opts::new("errors_total", "Total number of errors")
                .namespace(service_name),
            &["type", "operation"],
        )?;
        registry.register(Box::new(errors_total.clone()))?;
        
        let business_operations_total = IntCounterVec::new(
            Opts::new("business_operations_total", "Total number of business operations")
                .namespace(service_name),
            &["operation", "status"],
        )?;
        registry.register(Box::new(business_operations_total.clone()))?;
        
        let business_operation_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "business_operation_duration_seconds",
                "Business operation duration in seconds",
            )
            .namespace(service_name)
            .buckets(vec![0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0]),
            &["operation"],
        )?;
        registry.register(Box::new(business_operation_duration_seconds.clone()))?;
        
        Ok(Self {
            http_request_counter,
            http_request_duration,
            http_requests_total,
            http_request_duration_seconds,
            http_requests_in_progress,
            errors_total,
            business_operations_total,
            business_operation_duration_seconds,
        })
    }
    
    /// Record an HTTP request
    pub fn record_http_request(
        &self,
        method: &str,
        endpoint: &str,
        status: u16,
        duration: f64,
    ) {
        self.http_requests_total
            .with_label_values(&[method, endpoint, &status.to_string()])
            .inc();
        
        self.http_request_duration_seconds
            .with_label_values(&[method, endpoint])
            .observe(duration);
    }
    
    /// Record an error
    pub fn record_error(&self, error_type: &str, operation: &str) {
        self.errors_total
            .with_label_values(&[error_type, operation])
            .inc();
    }
    
    /// Record a business operation
    pub fn record_business_operation(&self, operation: &str, status: &str, duration: f64) {
        self.business_operations_total
            .with_label_values(&[operation, status])
            .inc();
        
        self.business_operation_duration_seconds
            .with_label_values(&[operation])
            .observe(duration);
    }
}

/// Timer to measure operation duration
pub struct MetricsTimer {
    start: Instant,
}

impl MetricsTimer {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
    
    pub fn elapsed_seconds(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

impl Default for MetricsTimer {
    fn default() -> Self {
        Self::new()
    }
}

/// Deposit service specific metrics
pub struct DepositMetrics {
    pub deposits_total: IntCounterVec,
    pub deposits_amount_satoshis: IntCounterVec,
    pub withdrawals_total: IntCounterVec,
    pub withdrawals_amount_satoshis: IntCounterVec,
    pub active_deposits: IntGauge,
}

impl DepositMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let deposits_total = IntCounterVec::new(
            Opts::new("deposits_total", "Total number of deposits"),
            &["status"],
        )?;
        registry.register(Box::new(deposits_total.clone()))?;
        
        let deposits_amount_satoshis = IntCounterVec::new(
            Opts::new("deposits_amount_satoshis_total", "Total amount deposited in satoshis"),
            &["status"],
        )?;
        registry.register(Box::new(deposits_amount_satoshis.clone()))?;
        
        let withdrawals_total = IntCounterVec::new(
            Opts::new("withdrawals_total", "Total number of withdrawals"),
            &["status"],
        )?;
        registry.register(Box::new(withdrawals_total.clone()))?;
        
        let withdrawals_amount_satoshis = IntCounterVec::new(
            Opts::new("withdrawals_amount_satoshis_total", "Total amount withdrawn in satoshis"),
            &["status"],
        )?;
        registry.register(Box::new(withdrawals_amount_satoshis.clone()))?;
        
        let active_deposits = IntGauge::new(
            "active_deposits",
            "Number of currently active deposits",
        )?;
        registry.register(Box::new(active_deposits.clone()))?;
        
        Ok(Self {
            deposits_total,
            deposits_amount_satoshis,
            withdrawals_total,
            withdrawals_amount_satoshis,
            active_deposits,
        })
    }
}

/// Lending service specific metrics
pub struct LendingMetrics {
    pub loans_total: IntCounterVec,
    pub loans_amount_satoshis: IntCounterVec,
    pub active_loans: IntGauge,
    pub liquidations_total: IntCounter,
}

impl LendingMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let loans_total = IntCounterVec::new(
            Opts::new("loans_total", "Total number of loans"),
            &["status"],
        )?;
        registry.register(Box::new(loans_total.clone()))?;
        
        let loans_amount_satoshis = IntCounterVec::new(
            Opts::new("loans_amount_satoshis_total", "Total amount loaned in satoshis"),
            &["status"],
        )?;
        registry.register(Box::new(loans_amount_satoshis.clone()))?;
        
        let active_loans = IntGauge::new(
            "active_loans",
            "Number of currently active loans",
        )?;
        registry.register(Box::new(active_loans.clone()))?;
        
        let liquidations_total = IntCounter::new(
            "liquidations_total",
            "Total number of loan liquidations",
        )?;
        registry.register(Box::new(liquidations_total.clone()))?;
        
        Ok(Self {
            loans_total,
            loans_amount_satoshis,
            active_loans,
            liquidations_total,
        })
    }
}

/// Payment channel specific metrics
pub struct ChannelMetrics {
    pub channels_total: IntCounterVec,
    pub channel_payments_total: IntCounter,
    pub channel_payments_amount_satoshis: IntCounter,
    pub active_channels: IntGauge,
}

impl ChannelMetrics {
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let channels_total = IntCounterVec::new(
            Opts::new("channels_total", "Total number of payment channels"),
            &["status"],
        )?;
        registry.register(Box::new(channels_total.clone()))?;
        
        let channel_payments_total = IntCounter::new(
            "channel_payments_total",
            "Total number of payments through channels",
        )?;
        registry.register(Box::new(channel_payments_total.clone()))?;
        
        let channel_payments_amount_satoshis = IntCounter::new(
            "channel_payments_amount_satoshis_total",
            "Total amount of payments through channels in satoshis",
        )?;
        registry.register(Box::new(channel_payments_amount_satoshis.clone()))?;
        
        let active_channels = IntGauge::new(
            "active_channels",
            "Number of currently active payment channels",
        )?;
        registry.register(Box::new(active_channels.clone()))?;
        
        Ok(Self {
            channels_total,
            channel_payments_total,
            channel_payments_amount_satoshis,
            active_channels,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_service_metrics_creation() {
        let registry = Registry::new();
        let metrics = ServiceMetrics::new(&registry, "test_service");
        assert!(metrics.is_ok());
    }
    
    #[test]
    fn test_record_http_request() {
        let registry = Registry::new();
        let metrics = ServiceMetrics::new(&registry, "test_service").unwrap();
        
        metrics.record_http_request("GET", "/test", 200, 0.1);
        
        // Verify counter increased
        let metric_families = registry.gather();
        assert!(!metric_families.is_empty());
    }
    
    #[test]
    fn test_record_error() {
        let registry = Registry::new();
        let metrics = ServiceMetrics::new(&registry, "test_service").unwrap();
        
        metrics.record_error("validation", "create_deposit");
        
        let metric_families = registry.gather();
        assert!(!metric_families.is_empty());
    }
    
    #[test]
    fn test_record_business_operation() {
        let registry = Registry::new();
        let metrics = ServiceMetrics::new(&registry, "test_service").unwrap();
        
        metrics.record_business_operation("deposit_created", "success", 0.5);
        
        let metric_families = registry.gather();
        assert!(!metric_families.is_empty());
    }
    
    #[test]
    fn test_metrics_timer() {
        let timer = MetricsTimer::new();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed = timer.elapsed_seconds();
        assert!(elapsed >= 0.01);
    }
    
    #[test]
    fn test_deposit_metrics_creation() {
        let registry = Registry::new();
        let metrics = DepositMetrics::new(&registry);
        assert!(metrics.is_ok());
    }
    
    #[test]
    fn test_lending_metrics_creation() {
        let registry = Registry::new();
        let metrics = LendingMetrics::new(&registry);
        assert!(metrics.is_ok());
    }
    
    #[test]
    fn test_channel_metrics_creation() {
        let registry = Registry::new();
        let metrics = ChannelMetrics::new(&registry);
        assert!(metrics.is_ok());
    }
    
    #[test]
    fn test_multiple_metrics_registration() {
        let registry = Registry::new();
        
        let service_metrics = ServiceMetrics::new(&registry, "test_service");
        assert!(service_metrics.is_ok());
        
        // Registering the same metrics again should fail
        let duplicate = ServiceMetrics::new(&registry, "test_service");
        assert!(duplicate.is_err());
    }
}