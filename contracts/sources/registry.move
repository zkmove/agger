module tds::registry {
    use std::string;
    use std::vector;

    use aptos_std::from_bcs;
    use aptos_std::table;
    use aptos_framework::account;
    use aptos_framework::event;
    use std::signer::address_of;

    struct ModuleId has copy, drop, store {
        addr: vector<u8>,
        name: string::String,
    }

    struct Modules has key {
        modules: table::Table<ModuleId, vector<u8>>,

    }

    struct Registry has key {
        verify_keys: table::Table<ModuleId, table::Table<u16, vector<u8>>>,
        event_handle: event::EventHandle<ModuleRegisterEvent>
    }

    // struct CircuitConfig has copy, drop {
    //     max_step_row: u32,
    //     stack_ops_num: u32,
    //     locals_ops_num: u32,
    //     global_ops_num: u32,
    //     word_size: u32,
    //     max_frame_index: u32,
    //     max_locals_size: u32,
    //     max_stack_size: u32,
    // }

    struct ModuleRegisterEvent has drop, store {
        module_id: ModuleId
    }

    fun init_module(account: &signer) {
        assert!(address_of(account) == @tds, 401);
        move_to(account, Modules { modules: table::new() });
        move_to(account, Registry {
            verify_keys: table::new(),
            event_handle: account::new_event_handle(account)
        });
    }

    #[view]
    public fun get_module(addr: vector<u8>, name: vector<u8>): vector<u8>
    acquires Modules {
        let id = ModuleId { addr, name: string::utf8(name) };
        let ms = borrow_global<Modules>(@tds);
        *table::borrow(&ms.modules, id)
    }

    #[view]
    public fun get_vk(addr: vector<u8>, name: vector<u8>, function_index: u16): vector<u8>
    acquires Registry {
        let id = ModuleId { addr, name: string::utf8(name) };
        let registry = borrow_global<Registry>(@tds);
        let mkeys = table::borrow(&registry.verify_keys, id);
        *table::borrow(mkeys, function_index)
    }

    /// verify_key is composed with vk+function_index+circuit_configs
    /// TODO: add circuit configuration.
    public entry fun register_module(
        addr: vector<u8>,
        name: vector<u8>,
        code: vector<u8>,
        func_verify_keys: vector<vector<u8>>
    )
    acquires Modules, Registry {
        let module_id = ModuleId { addr, name: string::utf8(name) };

        add_module(module_id, code);
        add_entry_function_verify_keys(module_id, func_verify_keys);
    }


    // todo: parse module id from bytecode ?
    public fun add_module(module_id: ModuleId, code: vector<u8>)
    acquires Modules {
        let modules = borrow_global_mut<Modules>(@tds);
        table::add(&mut modules.modules, module_id, code);
    }

    public fun add_entry_function_verify_keys(module_id: ModuleId, verify_keys: vector<vector<u8>>)
    acquires Registry {
        let registry = borrow_global_mut<Registry>(@tds);
        let i = vector::length(&verify_keys);
        while (i > 0) {
            i = i - 1;

            let vk = vector::pop_back(&mut verify_keys);
            // le encoding of function index
            let new_len =vector::length(&vk) - 2;
            let func_index = from_bcs::to_u16(vector::trim(&mut vk, new_len));
            add_entry_function_verify_key(&mut registry.verify_keys, module_id, func_index, vk);
        };
        event::emit_event(&mut registry.event_handle, ModuleRegisterEvent { module_id });
    }

    fun add_entry_function_verify_key(
        verify_keys: &mut table::Table<ModuleId, table::Table<u16, vector<u8>>>,
        module_id: ModuleId,
        function_index: u16,
        vk: vector<u8>
    ) {
        if (table::contains(verify_keys, module_id)) {
            let t = table::borrow_mut(verify_keys, module_id);
            table::add(t, function_index, vk);
        } else {
            let t = table::new();
            table::add(&mut t, function_index, vk);
            table::add(verify_keys, module_id, t);
        }
    }
}