//! FIXT 1.1 transport/application dictionary separation.
//!
//! Under FIXT 1.1 the session (transport) layer and the application layer use **separate**
//! dictionaries. The application dictionary is selected per-message by ApplVerID, falling back to
//! the session's DefaultApplVerID.

use std::collections::BTreeMap;

use crate::model::DataDictionary;

/// A transport dictionary plus one or more application dictionaries keyed by application version.
#[derive(Debug, Clone)]
pub struct FixtDictionaries {
    transport: DataDictionary,
    applications: BTreeMap<String, DataDictionary>,
    default_appl_ver_id: Option<String>,
}

impl FixtDictionaries {
    /// Create from a transport dictionary; add application dictionaries with
    /// [`with_application`](Self::with_application).
    pub fn new(transport: DataDictionary) -> Self {
        Self {
            transport,
            applications: BTreeMap::new(),
            default_appl_ver_id: None,
        }
    }

    /// Register an application dictionary under an application version id (e.g. `"FIX.5.0"`).
    pub fn with_application(
        mut self,
        appl_ver_id: impl Into<String>,
        dict: DataDictionary,
    ) -> Self {
        self.applications.insert(appl_ver_id.into(), dict);
        self
    }

    /// Set the DefaultApplVerID used when a message does not carry an explicit ApplVerID.
    pub fn with_default_appl_ver_id(mut self, appl_ver_id: impl Into<String>) -> Self {
        self.default_appl_ver_id = Some(appl_ver_id.into());
        self
    }

    /// The transport (session-layer) dictionary.
    pub fn transport(&self) -> &DataDictionary {
        &self.transport
    }

    /// Resolve the application dictionary for an explicit `appl_ver_id`, falling back to the
    /// DefaultApplVerID.
    pub fn application_for(&self, appl_ver_id: Option<&str>) -> Option<&DataDictionary> {
        let key = appl_ver_id.or(self.default_appl_ver_id.as_deref())?;
        self.applications.get(key)
    }
}
