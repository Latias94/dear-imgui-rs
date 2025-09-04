//! Text filtering utilities for Dear ImGui
//! 
//! This module provides text filtering functionality that can be used to
//! filter lists of items based on user input.

use std::collections::HashSet;

/// A text filter that can be used to filter lists of items
/// 
/// The filter supports multiple search terms separated by commas,
/// and can exclude items by prefixing terms with '-'.
/// 
/// # Examples
/// 
/// ```rust
/// use dear_imgui::TextFilter;
/// 
/// let mut filter = TextFilter::new();
/// filter.set_filter("hello,world,-exclude");
/// 
/// assert!(filter.pass_filter("hello there"));
/// assert!(filter.pass_filter("world peace"));
/// assert!(!filter.pass_filter("exclude this"));
/// assert!(!filter.pass_filter("nothing"));
/// ```
#[derive(Debug, Clone)]
pub struct TextFilter {
    /// The current filter string
    filter: String,
    /// Parsed include terms (must contain at least one)
    include_terms: Vec<String>,
    /// Parsed exclude terms (must not contain any)
    exclude_terms: Vec<String>,
    /// Whether the filter is case sensitive
    case_sensitive: bool,
}

impl Default for TextFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextFilter {
    /// Create a new empty text filter
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let filter = TextFilter::new();
    /// assert!(filter.is_empty());
    /// ```
    pub fn new() -> Self {
        Self {
            filter: String::new(),
            include_terms: Vec::new(),
            exclude_terms: Vec::new(),
            case_sensitive: false,
        }
    }
    
    /// Create a new text filter with the given filter string
    /// 
    /// # Arguments
    /// 
    /// * `filter` - The initial filter string
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let filter = TextFilter::with_filter("hello,world");
    /// assert!(filter.pass_filter("hello there"));
    /// ```
    pub fn with_filter(filter: impl AsRef<str>) -> Self {
        let mut text_filter = Self::new();
        text_filter.set_filter(filter);
        text_filter
    }
    
    /// Set the filter string and parse it
    /// 
    /// The filter string can contain multiple terms separated by commas.
    /// Terms prefixed with '-' are treated as exclude terms.
    /// 
    /// # Arguments
    /// 
    /// * `filter` - The filter string to parse
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let mut filter = TextFilter::new();
    /// filter.set_filter("include1,include2,-exclude1,-exclude2");
    /// ```
    pub fn set_filter(&mut self, filter: impl AsRef<str>) {
        let filter = filter.as_ref();
        self.filter = filter.to_string();
        self.parse_filter();
    }
    
    /// Get the current filter string
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let mut filter = TextFilter::new();
    /// filter.set_filter("hello,world");
    /// assert_eq!(filter.get_filter(), "hello,world");
    /// ```
    pub fn get_filter(&self) -> &str {
        &self.filter
    }
    
    /// Check if the filter is empty
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let filter = TextFilter::new();
    /// assert!(filter.is_empty());
    /// 
    /// let filter = TextFilter::with_filter("hello");
    /// assert!(!filter.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.include_terms.is_empty() && self.exclude_terms.is_empty()
    }
    
    /// Set whether the filter should be case sensitive
    /// 
    /// # Arguments
    /// 
    /// * `case_sensitive` - Whether to use case sensitive matching
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let mut filter = TextFilter::with_filter("Hello");
    /// filter.set_case_sensitive(true);
    /// assert!(!filter.pass_filter("hello")); // Case sensitive
    /// 
    /// filter.set_case_sensitive(false);
    /// assert!(filter.pass_filter("hello")); // Case insensitive
    /// ```
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        if self.case_sensitive != case_sensitive {
            self.case_sensitive = case_sensitive;
            self.parse_filter(); // Re-parse with new case sensitivity
        }
    }
    
    /// Check if the filter is case sensitive
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let filter = TextFilter::new();
    /// assert!(!filter.is_case_sensitive()); // Default is case insensitive
    /// ```
    pub fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }
    
    /// Test if the given text passes the filter
    /// 
    /// Returns `true` if the text should be included based on the current filter.
    /// 
    /// # Arguments
    /// 
    /// * `text` - The text to test against the filter
    /// 
    /// # Returns
    /// 
    /// `true` if the text passes the filter, `false` otherwise
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let filter = TextFilter::with_filter("hello,world,-exclude");
    /// 
    /// assert!(filter.pass_filter("hello there"));
    /// assert!(filter.pass_filter("world peace"));
    /// assert!(!filter.pass_filter("exclude this"));
    /// assert!(!filter.pass_filter("nothing matches"));
    /// ```
    pub fn pass_filter(&self, text: impl AsRef<str>) -> bool {
        let text = text.as_ref();
        
        // If no filter is set, everything passes
        if self.is_empty() {
            return true;
        }
        
        let text_to_check = if self.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };
        
        // Check exclude terms first - if any match, reject
        for exclude_term in &self.exclude_terms {
            if text_to_check.contains(exclude_term) {
                return false;
            }
        }
        
        // If no include terms, and we passed exclude check, accept
        if self.include_terms.is_empty() {
            return true;
        }
        
        // Check include terms - at least one must match
        for include_term in &self.include_terms {
            if text_to_check.contains(include_term) {
                return true;
            }
        }
        
        false
    }
    
    /// Clear the filter
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let mut filter = TextFilter::with_filter("hello");
    /// assert!(!filter.is_empty());
    /// 
    /// filter.clear();
    /// assert!(filter.is_empty());
    /// ```
    pub fn clear(&mut self) {
        self.filter.clear();
        self.include_terms.clear();
        self.exclude_terms.clear();
    }
    
    /// Get the number of include terms
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let filter = TextFilter::with_filter("hello,world,test");
    /// assert_eq!(filter.include_count(), 3);
    /// ```
    pub fn include_count(&self) -> usize {
        self.include_terms.len()
    }
    
    /// Get the number of exclude terms
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use dear_imgui::TextFilter;
    /// 
    /// let filter = TextFilter::with_filter("hello,-exclude1,-exclude2");
    /// assert_eq!(filter.exclude_count(), 2);
    /// ```
    pub fn exclude_count(&self) -> usize {
        self.exclude_terms.len()
    }
    
    /// Parse the filter string into include and exclude terms
    fn parse_filter(&mut self) {
        self.include_terms.clear();
        self.exclude_terms.clear();
        
        if self.filter.is_empty() {
            return;
        }
        
        // Split by comma and process each term
        for term in self.filter.split(',') {
            let term = term.trim();
            if term.is_empty() {
                continue;
            }
            
            if let Some(exclude_term) = term.strip_prefix('-') {
                // Exclude term
                let exclude_term = exclude_term.trim();
                if !exclude_term.is_empty() {
                    let processed_term = if self.case_sensitive {
                        exclude_term.to_string()
                    } else {
                        exclude_term.to_lowercase()
                    };
                    self.exclude_terms.push(processed_term);
                }
            } else {
                // Include term
                let processed_term = if self.case_sensitive {
                    term.to_string()
                } else {
                    term.to_lowercase()
                };
                self.include_terms.push(processed_term);
            }
        }
        
        // Remove duplicates while preserving order
        self.include_terms = self.include_terms
            .iter()
            .fold((Vec::new(), HashSet::new()), |(mut vec, mut set), item| {
                if set.insert(item.clone()) {
                    vec.push(item.clone());
                }
                (vec, set)
            }).0;
            
        self.exclude_terms = self.exclude_terms
            .iter()
            .fold((Vec::new(), HashSet::new()), |(mut vec, mut set), item| {
                if set.insert(item.clone()) {
                    vec.push(item.clone());
                }
                (vec, set)
            }).0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_filter() {
        let filter = TextFilter::new();
        assert!(filter.is_empty());
        assert!(filter.pass_filter("anything"));
        assert_eq!(filter.get_filter(), "");
    }

    #[test]
    fn test_include_terms() {
        let filter = TextFilter::with_filter("hello,world");
        assert!(!filter.is_empty());
        assert_eq!(filter.include_count(), 2);
        assert_eq!(filter.exclude_count(), 0);
        
        assert!(filter.pass_filter("hello there"));
        assert!(filter.pass_filter("world peace"));
        assert!(filter.pass_filter("hello world"));
        assert!(!filter.pass_filter("nothing"));
    }

    #[test]
    fn test_exclude_terms() {
        let filter = TextFilter::with_filter("-exclude,-bad");
        assert!(!filter.is_empty());
        assert_eq!(filter.include_count(), 0);
        assert_eq!(filter.exclude_count(), 2);
        
        assert!(filter.pass_filter("good text"));
        assert!(!filter.pass_filter("exclude this"));
        assert!(!filter.pass_filter("bad news"));
    }

    #[test]
    fn test_mixed_terms() {
        let filter = TextFilter::with_filter("good,nice,-bad,-exclude");
        assert_eq!(filter.include_count(), 2);
        assert_eq!(filter.exclude_count(), 2);
        
        assert!(filter.pass_filter("good news"));
        assert!(filter.pass_filter("nice day"));
        assert!(!filter.pass_filter("bad news"));
        assert!(!filter.pass_filter("exclude this"));
        assert!(!filter.pass_filter("nothing"));
    }

    #[test]
    fn test_case_sensitivity() {
        let mut filter = TextFilter::with_filter("Hello");
        
        // Case insensitive (default)
        assert!(!filter.is_case_sensitive());
        assert!(filter.pass_filter("hello"));
        assert!(filter.pass_filter("HELLO"));
        assert!(filter.pass_filter("Hello"));
        
        // Case sensitive
        filter.set_case_sensitive(true);
        assert!(filter.is_case_sensitive());
        assert!(!filter.pass_filter("hello"));
        assert!(!filter.pass_filter("HELLO"));
        assert!(filter.pass_filter("Hello"));
    }

    #[test]
    fn test_clear() {
        let mut filter = TextFilter::with_filter("hello,world,-exclude");
        assert!(!filter.is_empty());
        
        filter.clear();
        assert!(filter.is_empty());
        assert_eq!(filter.get_filter(), "");
        assert_eq!(filter.include_count(), 0);
        assert_eq!(filter.exclude_count(), 0);
    }

    #[test]
    fn test_whitespace_handling() {
        let filter = TextFilter::with_filter(" hello , world , -exclude ");
        assert_eq!(filter.include_count(), 2);
        assert_eq!(filter.exclude_count(), 1);
        
        assert!(filter.pass_filter("hello there"));
        assert!(filter.pass_filter("world peace"));
        assert!(!filter.pass_filter("exclude this"));
    }

    #[test]
    fn test_empty_terms() {
        let filter = TextFilter::with_filter("hello,,,-,world");
        assert_eq!(filter.include_count(), 2);
        assert_eq!(filter.exclude_count(), 0);
        
        assert!(filter.pass_filter("hello"));
        assert!(filter.pass_filter("world"));
    }

    #[test]
    fn test_duplicate_terms() {
        let filter = TextFilter::with_filter("hello,hello,world,hello,-bad,-bad");
        assert_eq!(filter.include_count(), 2); // Duplicates removed
        assert_eq!(filter.exclude_count(), 1); // Duplicates removed
    }
}
