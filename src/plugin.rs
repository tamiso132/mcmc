use flecs_ecs::core::flecs::Singleton;
use flecs_ecs::prelude::*;
use std::any::{Any, TypeId};
use std::collections::HashMap;

// --- ImportFlags (replaces enum class and bitwise ops) ---

use bitflags::bitflags;

bitflags! {
    /// Controls what parts of a plugin are imported.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    #[repr(transparent)]
    pub struct ImportFlags: u32 {
        /// Enables system imports.
        const ImportSystems = 1;
        /// The default, matching the C++ default.
        const All = Self::ImportSystems.bits();
    }
}

// --- PluginInterface (replaces C++ concept) ---

/// Defines the interface for a Flecs plugin.
pub trait PluginInterface {
    /// Arguments used to register the plugin.
    /// Plugins without args must explicitly set this to `()`.
    type Args;

    /// The main registration function for the plugin.
    /// Called once by the `PluginManager`.
    fn register_plugin(world: &World, args: Self::Args);

    /// Static function to import components.
    /// Called by the `PluginManager`.
    fn import_components(world: &World);

    /// Static function to import systems.
    /// Called by the `PluginManager` if the `ImportSystems` flag is set.
    fn import_systems(world: &World);

    /// Static function called on plugin shutdown.
    fn shutdown(world: &World);
}

// --- Plugin (replaces C++ struct) ---

// Type aliases for the function pointers
type FnImport = fn(&World);
type FnShutdown = fn(&World);

/// A type-erased wrapper for a `PluginInterface` implementation.
pub struct Plugin {
    import_component_ptr: FnImport,
    import_system_ptr: FnImport,
    shutdown_ptr: FnShutdown,
    type_id: TypeId,
    type_name: &'static str,
}

impl Plugin {
    /// Creates a new type-erased plugin from a concrete `PluginInterface` implementor.
    pub fn new<T: PluginInterface + 'static>() -> Self {
        Self {
            import_component_ptr: T::import_components,
            import_system_ptr: T::import_systems,
            shutdown_ptr: T::shutdown,
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
        }
    }

    /// Calls the plugin's `import_systems` function.
    pub fn import_systems(&self, ecs: &World) {
        (self.import_system_ptr)(ecs);
    }

    /// Calls the plugin's `import_components` function.
    pub fn import_components(&self, ecs: &World) {
        (self.import_component_ptr)(ecs);
    }

    /// Calls the plugin's `shutdown` function.
    pub fn shutdown(&self, ecs: &World) {
        (self.shutdown_ptr)(ecs);
    }

    /// Gets the string name of the original plugin type.
    pub fn get_name(&self) -> &'static str {
        self.type_name
    }
}

// --- PluginManager (replaces C++ struct) ---

/// Manages the registration and lifecycle of plugins.
#[derive(Default)]
pub struct PluginManager {
    /// Stores the type-erased plugin instances.
    instances: Vec<Plugin>,
    /// Maps `TypeId` to the index in the `instances` vector.
    data_index: HashMap<TypeId, usize>,
}

impl PluginManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a plugin with the world.
    ///
    /// This function will:
    /// 1. Call `T::import_components`.
    /// 2. Conditionally call `T::import_systems` based on `flags`.
    /// 3. Call `T::register_plugin` with the provided `args`.
    ///
    /// Returns the index of the registered plugin.
    pub fn register_plugin<T: PluginInterface + 'static>(
        &mut self,
        ecs: &World, // Changed to &World
        args: T::Args,
        flags: Option<ImportFlags>,
    ) -> usize {
        let flags = flags.unwrap_or(ImportFlags::All);

        // Create the type-erased wrapper
        let plugin = Plugin::new::<T>();

        // Call the static import functions
        plugin.import_components(ecs);
        if flags.contains(ImportFlags::ImportSystems) {
            log::info!("import systems?");
            plugin.import_systems(ecs);
        }

        // Store the plugin
        let type_id = plugin.type_id;
        self.instances.push(plugin);
        let idx = self.instances.len() - 1;
        self.data_index.insert(type_id, idx);

        // Call the plugin's main registration logic
        T::register_plugin(ecs, args);

        idx
    }

    /// Gets a reference to a registered plugin by its type.
    pub fn get_plugin<T: 'static>(&self) -> Option<&Plugin> {
        self.data_index
            .get(&TypeId::of::<T>())
            .map(|&i| &self.instances[i])
    }
}

// --- Helper Functions ---

/// Registers a component as a singleton and sets its default value.
pub fn reg_singleton_default<T>(world: &World)
// Changed to &World
where
    T: ComponentId + DataComponent + ComponentType<Struct> + Default, // Simplified bounds
{
    reg_singleton::<T>(world);
    world.set::<T>(T::default());
}

/// Registers a component as a singleton.
pub fn reg_singleton<T: ComponentId>(world: &World) {
    // Changed to &World
    world.component::<T>().add(Singleton);
}

/// Registers a component with the world.
pub fn reg_comp<T: ComponentId>(world: &World) {
    // Changed to &World
    world.component::<T>();
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use flecs_ecs::prelude::*;

    // --- Test Components & Tags ---
    // `#[derive(Component)]` is all that's needed.
    // The manual `impl`s are redundant and were removed.

    #[derive(Default, Debug, PartialEq, Clone, Component)]
    #[repr(C)]
    struct TestSingleton {
        value: i32,
    }

    #[derive(Default, Debug, PartialEq, Clone, Component)]
    #[repr(C)]
    struct TestComponent {
        x: f32,
        y: f32,
    }

    #[derive(Default, Debug, PartialEq, Clone, Copy, Component)]
    #[repr(C)]
    struct TestTag;

    #[derive(Default, Debug, PartialEq, Clone, Copy, Component)]
    #[repr(C)]
    struct SystemTag;

    // --- Test Plugin (No Arguments) ---

    struct PluginNoArgs;
    impl PluginInterface for PluginNoArgs {
        type Args = ();

        fn register_plugin(world: &World, _args: Self::Args) {
            reg_comp::<TestTag>(world);
        }

        fn import_components(world: &World) {
            reg_comp::<TestComponent>(world);
        }

        fn import_systems(world: &World) {
            reg_comp::<SystemTag>(world);
        }

        fn shutdown(_world: &World) {}
    }

    // --- Test Plugin (With Arguments) ---

    struct PluginArgs {
        value: i32,
    }

    struct PluginWithArgs;
    impl PluginInterface for PluginWithArgs {
        type Args = PluginArgs;

        fn register_plugin(world: &World, args: Self::Args) {
            // Use args to set a singleton's value
            reg_singleton_default::<TestSingleton>(world); // Use default helper
            world.set(TestSingleton { value: args.value });
        }

        fn import_components(world: &World) {
            reg_comp::<TestSingleton>(world);
        }

        fn import_systems(_world: &World) {}
        fn shutdown(_world: &World) {}
    }

    // --- Helper Function Tests ---

    #[test]
    fn test_reg_comp() {
        let world = World::new(); // No `mut` needed
        reg_comp::<TestComponent>(&world);
        let comp = world.component::<TestComponent>();
        assert!(comp.is_valid(), "Component was not registered");
    }

    #[test]
    fn test_reg_singleton() {
        let world = World::new(); // No `mut` needed
        reg_singleton::<TestSingleton>(&world);
        let comp = world.component::<TestSingleton>();
        assert!(comp.is_valid(), "Singleton component was not registered");
        assert!(comp.has(Singleton), "Component does not have Singleton tag");
    }

    #[test]
    fn test_reg_singleton_default() {
        let world = World::new(); // No `mut` needed
        reg_singleton_default::<TestSingleton>(&world);

        let comp = world.component::<TestSingleton>();
        assert!(comp.is_valid(), "Singleton component was not registered");
        assert!(comp.has(Singleton), "Component does not have Singleton tag");

        // --- FIXED LOGIC ---
        // Use `cloned` to get the value, as shown in the flecs docs.
        let singleton_val = world.cloned::<&TestSingleton>();
        assert_eq!(
            singleton_val.value,
            TestSingleton::default().value,
            "Singleton value was not set to default"
        );
    }

    // --- PluginManager Tests ---

    #[test]
    fn test_register_plugin_no_args() {
        let world = World::new(); // No `mut` needed
        let mut manager = PluginManager::new();

        let idx = manager.register_plugin::<PluginNoArgs>(&world, (), None);
        assert_eq!(idx, 0, "Plugin index should be 0");

        // Check functions were called
        assert!(
            world.component::<TestTag>().is_valid(),
            "`register_plugin` was not called"
        );
        assert!(
            world.component::<TestComponent>().is_valid(),
            "`import_components` was not called"
        );
        assert!(
            world.component::<SystemTag>().is_valid(),
            "`import_systems` was not called"
        );
    }

    #[test]
    fn test_register_plugin_with_args() {
        let world = World::new(); // No `mut` needed
        let mut manager = PluginManager::new();

        let args = PluginArgs { value: 42 };
        manager.register_plugin::<PluginWithArgs>(&world, args, None);

        // --- FIXED LOGIC ---
        // Use `cloned` to get the value and check it.
        let singleton_val = world.cloned::<&TestSingleton>();
        assert_eq!(singleton_val.value, 42, "Plugin did not use args correctly");
    }

    #[test]
    fn test_register_plugin_flags_components_only() {
        let world = World::new(); // No `mut` needed
        let mut manager = PluginManager::new();

        let flags = ImportFlags::empty(); // This is 0, which means no systems
        manager.register_plugin::<PluginNoArgs>(&world, (), Some(flags));

        assert!(
            world.component::<TestComponent>().is_valid(),
            "`import_components` was not called"
        );

        assert!(
            !world.component::<SystemTag>().is_valid(),
            "`import_systems` was called, but should have been skipped"
        );
    }

    #[test]
    fn test_register_plugin_flags_all() {
        let world = World::new(); // No `mut` needed
        let mut manager = PluginManager::new();

        let flags = ImportFlags::All;
        manager.register_plugin::<PluginNoArgs>(&world, (), Some(flags));

        assert!(
            world.component::<TestComponent>().is_valid(),
            "`import_components` was not called"
        );
        assert!(
            world.component::<SystemTag>().is_valid(),
            "`import_systems` was not called"
        );
    }

    #[test]
    fn test_get_plugin() {
        let world = World::new(); // No `mut` needed
        let mut manager = PluginManager::new();

        manager.register_plugin::<PluginNoArgs>(&world, (), None);
        manager.register_plugin::<PluginWithArgs>(&world, PluginArgs { value: 10 }, None);

        let plugin1 = manager.get_plugin::<PluginNoArgs>();
        assert!(plugin1.is_some());
        assert!(plugin1.unwrap().get_name().contains("PluginNoArgs"));

        let plugin2 = manager.get_plugin::<PluginWithArgs>();
        assert!(plugin2.is_some());
        assert!(plugin2.unwrap().get_name().contains("PluginWithArgs"));

        let plugin_none = manager.get_plugin::<TestComponent>();
        assert!(plugin_none.is_none());
    }
}
