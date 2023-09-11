module tds::registry {
    use std::string;
    use std::vector;

    use aptos_std::table;
    use aptos_framework::account;
    use aptos_framework::event;

    struct ModuleId has copy, drop, store {
        addr: address,
        name: string::String,
    }

    struct Modules has key {
        modules: table::Table<ModuleId, vector<u8>>,

    }

    struct Registry has key {
        verify_keys: table::Table<ModuleId, table::Table<u16, vector<u8>>>,
        event_handle: event::EventHandle<ModuleRegisterEvent>
    }

    struct ModuleRegisterEvent has drop, store {
        module_id: ModuleId
    }

    fun init_module(account: &signer) {
        move_to(account, Modules { modules: table::new() });
        move_to(account, Registry {
            verify_keys: table::new(),
            event_handle: account::new_event_handle(account)
        });
    }

    #[view]
    public fun get_module(addr: address, name: vector<u8>): vector<u8>
    acquires Modules {
        let id = ModuleId { addr, name: string::utf8(name) };
        let ms = borrow_global<Modules>(@tds);
        *table::borrow(&ms.modules, id)
    }

    #[view]
    public fun get_vk(addr: address, name: vector<u8>, function_index: u16): vector<u8>
    acquires Registry {
        let id = ModuleId { addr, name: string::utf8(name) };
        let registry = borrow_global<Registry>(@tds);
        let mkeys = table::borrow(&registry.verify_keys, id);
        *table::borrow(mkeys, function_index)
    }

    /// verify_key is composed with vk+function_index
    /// TODO: add circuit configuration.
    public entry fun register_module(addr: address, name: vector<u8>, code: vector<u8>, verify_keys: vector<vector<u8>>)
    acquires Modules, Registry {
        let module_id = ModuleId { addr, name: string::utf8(name) };

        add_module(module_id, code);
        add_entry_function_verify_keys(module_id, verify_keys);
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
            // bigendian encoding of function index
            let lo = vector::pop_back(&mut vk);
            let hi = vector::pop_back(&mut vk);
            let func_index = ((hi as u16) << 8) + (lo as u16);
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