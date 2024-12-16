use dark_std::sync::SyncVec;

///log filter
pub trait Filter: Send + Sync {
    /// if return true=do_log/false=not_log
    fn do_log(&self, record: &log::Record) -> bool;
}

/// an Module Filter
/// ```rust
/// fn main(){
///    use fastlog::Config;
///    use fastlog::filter::ModuleFilter;
///    let filter = ModuleFilter::new();
///    filter.modules.push(module_path!().to_string());
///    fastlog::init(Config::new().console().add_filter(filter)).unwrap();
/// }
/// ```
pub struct ModuleFilter {
    pub modules: SyncVec<String>,
}

impl ModuleFilter {
    pub fn new() -> Self {
        Self { modules: SyncVec::new() }
    }
}

impl Filter for ModuleFilter {
    fn do_log(&self, record: &log::Record) -> bool {
        let module = record.module_path().unwrap_or("");
        if !self.modules.is_empty() {
            for x in &self.modules {
                if module == x {
                    return false;
                }
            }
        }
        return true;
    }
}
