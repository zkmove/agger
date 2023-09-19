module agger::query {
    use std::option::{Self, Option};
    use std::signer::address_of;

    use aptos_std::table_with_length;
    use aptos_framework::account;
    use aptos_framework::event;

    struct Query has store {
        module_address: vector<u8>,
        module_name: vector<u8>,
        function_index: u16,
        // arg encoded in str, see parse_transaction_arguments
        args: vector<vector<u8>>,
        // move type in str, see parse_type_tag
        ty_args: vector<vector<u8>>,
        deadline: u64,
        success: Option<bool>,
        result: Option<vector<u8>>,
    }

    struct Queries has key {
        query_counter: u64,
        queries: table_with_length::TableWithLength<u64, Query>,
    }

    struct EventHandles has key {
        new_event_handle: event::EventHandle<NewQueryEvent>,
        reply_event_handle: event::EventHandle<ReplyQueryEvent>,
    }

    struct NewQueryEvent has drop, store {
        user: address,
        id: u64
    }

    struct ReplyQueryEvent has drop, store {
        user: address,
        id: u64
    }


    fun init_module(sender: &signer) {
        assert!(address_of(sender) == @agger, 401);
        move_to(sender, EventHandles {
            new_event_handle: account::new_event_handle(sender),
            reply_event_handle: account::new_event_handle(sender),
        });
    }

    public entry fun send_query(
        sender: signer,
        module_address: vector<u8>,
        module_name: vector<u8>,
        function_index: u16,
        args: vector<vector<u8>>,
        ty_args: vector<vector<u8>>,
        deadline: u64
    ) acquires Queries, EventHandles {
        let sender_address = address_of(&sender);
        if (!exists<Queries>(sender_address)) {
            move_to(&sender, Queries {
                queries: table_with_length::new(),
                query_counter: 0,
            });
        };
        let queries = borrow_global_mut<Queries>(sender_address);
        let id = add_query(queries, Query {
            module_address, module_name, function_index, args, ty_args, deadline,
            success: option::none(),
            result: option::none()
        });
        let event_handles = borrow_global_mut<EventHandles>(@agger);
        event::emit_event(&mut event_handles.new_event_handle, NewQueryEvent { user: sender_address, id });
    }

    fun add_query(
        queries: &mut Queries,
        query: Query
    ): u64 {
        let id = queries.query_counter;
        table_with_length::add(&mut queries.queries, id, query);
        queries.query_counter = id + 1;
        id
    }

    public entry fun reply_query(
        sender: signer,
        user: address,
        query_id: u64,
        success: bool,
        result: vector<u8>,
        proof: vector<u8>
    ) acquires Queries, EventHandles {
        // todo: assert sender is prover?
        // todo: if success, validate proof. if proof is valid write the result to Query. else, do nothing.
        // todo: if failure, write result to Query.
        // if it's deadline, failure.
        let queries = borrow_global_mut<Queries>(user);
        let q = table_with_length::borrow_mut(&mut queries.queries, query_id);

        write_query_result(q, success, result);

        let event_handles = borrow_global_mut<EventHandles>(@agger);
        event::emit_event(&mut event_handles.reply_event_handle, ReplyQueryEvent { user, id: query_id });
    }

    fun write_query_result(q: &mut Query, success: bool, result: vector<u8>) {
        option::fill(&mut q.success, success);
        option::fill(&mut q.result, result);
    }
}
