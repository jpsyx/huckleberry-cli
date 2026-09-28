//! Structured queries: the `where` and `order by` of a Firestore read.
//!
//! Huckleberry asks the same question of four collections: give me the rows
//! whose `start` falls in a window, and separately give me the batched
//! multi-entry documents, which cannot be filtered because their timestamps
//! live inside a nested map. Both are a `runQuery` against one subcollection,
//! so the builder here is deliberately small: equality, a lower bound, an
//! upper bound, and an ascending sort.
//!
//! Firestore requires that a query with an inequality sort first by the field
//! the inequality is on. Every range read in this crate orders by `start`,
//! which is the field it bounds, so that requirement is met by construction.

use serde_json::{Value as Json, json};

use super::value;

/// A comparison in a `where` clause.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    /// `==`
    Equal,
    /// `<`
    LessThan,
    /// `>=`
    GreaterThanOrEqual,
}

impl Op {
    /// The name Firestore gives this comparison on the wire.
    #[must_use]
    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Equal => "EQUAL",
            Self::LessThan => "LESS_THAN",
            Self::GreaterThanOrEqual => "GREATER_THAN_OR_EQUAL",
        }
    }
}

/// One comparison against one field.
#[derive(Debug, Clone)]
struct Filter {
    field: String,
    op: Op,
    value: Json,
}

/// A query against one subcollection.
#[derive(Debug, Clone)]
pub struct Query {
    collection: String,
    filters: Vec<Filter>,
    order_by: Option<String>,
}

impl Query {
    /// Every document in a subcollection, unfiltered and unsorted.
    #[must_use]
    pub fn on(collection: &str) -> Self {
        Self {
            collection: collection.to_owned(),
            filters: Vec::new(),
            order_by: None,
        }
    }

    /// Narrows the query. Filters combine with AND, as Firestore does.
    #[must_use]
    pub fn filter(mut self, field: &str, op: Op, value: Json) -> Self {
        self.filters.push(Filter {
            field: field.to_owned(),
            op,
            value,
        });
        self
    }

    /// Sorts ascending by one field.
    #[must_use]
    pub fn order_by(mut self, field: &str) -> Self {
        self.order_by = Some(field.to_owned());
        self
    }
}

/// The request body `runQuery` takes.
#[must_use]
pub fn body(query: &Query) -> Json {
    let mut structured = json!({ "from": [{ "collectionId": query.collection }] });
    if let Some(clause) = where_clause(&query.filters) {
        structured["where"] = clause;
    }
    if let Some(field) = &query.order_by {
        structured["orderBy"] = json!([{
            "field": { "fieldPath": field },
            "direction": "ASCENDING",
        }]);
    }
    json!({ "structuredQuery": structured })
}

/// One filter goes out bare; several are ANDed together; none is no clause at
/// all, because Firestore rejects an empty composite.
fn where_clause(filters: &[Filter]) -> Option<Json> {
    match filters {
        [] => None,
        [only] => Some(field_filter(only)),
        many => Some(json!({ "compositeFilter": {
            "op": "AND",
            "filters": many.iter().map(field_filter).collect::<Vec<_>>(),
        } })),
    }
}

fn field_filter(filter: &Filter) -> Json {
    json!({ "fieldFilter": {
        "field": { "fieldPath": filter.field },
        "op": filter.op.wire_name(),
        "value": value::from_json(&filter.value),
    } })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_query_orders_by_the_field_it_bounds() {
        let query = Query::on("intervals")
            .filter("start", Op::GreaterThanOrEqual, json!(100))
            .filter("start", Op::LessThan, json!(200))
            .order_by("start");
        assert_eq!(
            body(&query)["structuredQuery"]["orderBy"],
            json!([{ "field": { "fieldPath": "start" }, "direction": "ASCENDING" }])
        );
    }

    #[test]
    fn two_filters_are_anded_together() {
        let query = Query::on("intervals")
            .filter("start", Op::GreaterThanOrEqual, json!(100))
            .filter("start", Op::LessThan, json!(200));
        let clause = &body(&query)["structuredQuery"]["where"]["compositeFilter"];
        assert_eq!(clause["op"], json!("AND"));
        assert_eq!(
            clause["filters"][0]["fieldFilter"]["op"],
            json!("GREATER_THAN_OR_EQUAL")
        );
        assert_eq!(
            clause["filters"][1]["fieldFilter"]["op"],
            json!("LESS_THAN")
        );
    }

    #[test]
    fn a_single_filter_needs_no_composite_wrapper() {
        let query = Query::on("intervals").filter("multi", Op::Equal, json!(true));
        assert_eq!(
            body(&query)["structuredQuery"]["where"],
            json!({ "fieldFilter": {
                "field": { "fieldPath": "multi" },
                "op": "EQUAL",
                "value": { "booleanValue": true },
            } })
        );
    }

    #[test]
    fn an_unfiltered_query_has_no_where_clause() {
        assert!(
            body(&Query::on("custom"))["structuredQuery"]
                .get("where")
                .is_none()
        );
    }

    #[test]
    fn the_bound_is_encoded_as_a_firestore_value() {
        let query = Query::on("intervals").filter("start", Op::LessThan, json!(1_758_572_400));
        assert_eq!(
            body(&query)["structuredQuery"]["where"]["fieldFilter"]["value"],
            json!({ "integerValue": "1758572400" })
        );
    }

    #[test]
    fn the_query_names_the_subcollection_it_reads() {
        assert_eq!(
            body(&Query::on("data"))["structuredQuery"]["from"],
            json!([{ "collectionId": "data" }])
        );
    }
}
