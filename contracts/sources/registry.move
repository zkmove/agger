module tds::registry {
    use std::string;
    use std::vector;
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

    struct VerificationParameters has copy, drop, store {
        /// circuit config in bcs
        config: vector<u8>,
        /// kzg param
        param: vector<u8>,
        /// verify key
        vk: vector<u8>,

    }

    struct Registry has key {
        verify_keys: table::Table<ModuleId, table::Table<u16, VerificationParameters>>,
        event_handle: event::EventHandle<ModuleRegisterEvent>
    }

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
        table::borrow(mkeys, function_index).vk
    }

    #[view]
    public fun get_param(addr: vector<u8>, name: vector<u8>, function_index: u16): vector<u8>
    acquires Registry {
        let id = ModuleId { addr, name: string::utf8(name) };
        let registry = borrow_global<Registry>(@tds);
        let mkeys = table::borrow(&registry.verify_keys, id);
        table::borrow(mkeys, function_index).param
    }

    #[view]
    public fun get_config(addr: vector<u8>, name: vector<u8>, function_index: u16): vector<u8>
    acquires Registry {
        let id = ModuleId { addr, name: string::utf8(name) };
        let registry = borrow_global<Registry>(@tds);
        let mkeys = table::borrow(&registry.verify_keys, id);
        table::borrow(mkeys, function_index).config
    }

    /// verify_key is composed with vk+function_index+circuit_configs
    /// TODO: add circuit configuration.
    public entry fun register_module(
        addr: vector<u8>,
        name: vector<u8>,
        code: vector<u8>,
        configs: vector<vector<u8>>,
        func_verify_keys: vector<vector<u8>>,
        params: vector<vector<u8>>,
    )
    acquires Modules, Registry {
        let module_id = ModuleId { addr, name: string::utf8(name) };

        add_module(module_id, code);
        add_entry_function_verify_keys(module_id, configs, func_verify_keys, params);
    }


    // todo: parse module id from bytecode ?
    public fun add_module(module_id: ModuleId, code: vector<u8>)
    acquires Modules {
        let modules = borrow_global_mut<Modules>(@tds);
        table::add(&mut modules.modules, module_id, code);
    }

    public fun add_entry_function_verify_keys(
        module_id: ModuleId,
        configs: vector<vector<u8>>,
        verify_keys: vector<vector<u8>>,
        params: vector<vector<u8>>
    )
    acquires Registry {
        let registry = borrow_global_mut<Registry>(@tds);
        let i = vector::length(&verify_keys);
        while (i > 0) {
            i = i - 1;
            let config = vector::pop_back(&mut configs);
            let vk = vector::pop_back(&mut verify_keys);
            let param = vector::pop_back(&mut params);

            // le encoding of function_index
            let hi = vector::pop_back(&mut vk);
            let lo = vector::pop_back(&mut vk);
            let func_index = (lo as u16) + ((hi as u16) << 8);
            add_entry_function_verify_key(&mut registry.verify_keys, module_id, func_index, config, vk, param);
        };
        event::emit_event(&mut registry.event_handle, ModuleRegisterEvent { module_id });
    }

    fun add_entry_function_verify_key(
        verify_keys: &mut table::Table<ModuleId, table::Table<u16, VerificationParameters>>,
        module_id: ModuleId,
        function_index: u16,
        config: vector<u8>,
        vk: vector<u8>,
        param: vector<u8>,
    ) {
        if (table::contains(verify_keys, module_id)) {
            let t = table::borrow_mut(verify_keys, module_id);
            table::add(t, function_index, VerificationParameters { config, vk, param });
        } else {
            let t = table::new();
            table::add(&mut t, function_index, VerificationParameters { config, vk, param });
            table::add(verify_keys, module_id, t);
        }
    }
}