table! {
    proposals (id) {
        id -> BigInt,
        title -> Nullable<Text>,
        summary -> Nullable<Text>,
        submit_output -> Nullable<Text>,
        executed_timestamp -> BigInt,
        failed_timestamp -> BigInt,
        failed -> Nullable<Text>,
    }
}

table! {
    subnet_update_nodes (id) {
        id -> Integer,
        subnet -> Text,
        in_progress -> Bool,
        nodes_to_add -> Nullable<Text>,
        nodes_to_remove -> Nullable<Text>,
        proposal_add_id -> Nullable<BigInt>,
        proposal_remove_id -> Nullable<BigInt>,
    }
}

allow_tables_to_appear_in_same_query!(
    proposals,
    subnet_update_nodes,
);
