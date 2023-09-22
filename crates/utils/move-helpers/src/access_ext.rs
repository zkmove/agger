use move_binary_format::{access::ModuleAccess, file_format::FunctionDefinition};
use move_core_types::identifier::IdentStr;

pub trait ModuleAccessExt: ModuleAccess {
    fn find_function_def_by_name(&self, name: &IdentStr) -> Option<&FunctionDefinition> {
        self.function_defs()
            .iter()
            .find(|fd| self.identifier_at(self.function_handle_at(fd.function).name) == name)
    }
}

impl<T> ModuleAccessExt for T where T: ModuleAccess {}
