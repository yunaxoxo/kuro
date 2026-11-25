//! A tiny host for running compiled kuro modules under `wasmtime`.
//!
//! It plays the role the browser plays in the playground: it provides the
//! `env.print_int` / `env.print_float` imports, calls exported functions and
//! reads linear memory. Only `tests/e2e_tests.rs` includes this file.

#![allow(dead_code)]

use wasmtime::{Caller, Engine, Instance, Linker, Module, Store, Trap, Val};

/// One call the program made to a print builtin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Printed {
    Int(i32),
    Float(f64),
}

/// Host state: everything the program printed, in order.
#[derive(Default)]
pub struct Host {
    pub printed: Vec<Printed>,
}

/// An instantiated module.
pub struct Vm {
    store: Store<Host>,
    instance: Instance,
}

impl Vm {
    /// Compile kuro `source` with *your* compiler and instantiate the
    /// result.
    pub fn from_source(source: &str) -> Vm {
        let wasm = kuro::compile(source)
            .unwrap_or_else(|err| panic!("unexpected compile error: {err}\n--- source ---\n{source}"));
        Vm::from_wasm(&wasm)
    }

    /// Instantiate a wasm binary, supplying the two host imports.
    pub fn from_wasm(wasm: &[u8]) -> Vm {
        let engine = Engine::default();
        let module = Module::new(&engine, wasm).unwrap_or_else(|err| {
            panic!("wasmtime rejected the module (it failed validation): {err:?}")
        });

        let mut linker: Linker<Host> = Linker::new(&engine);
        linker
            .func_wrap("env", "print_int", |mut caller: Caller<'_, Host>, value: i32| {
                caller.data_mut().printed.push(Printed::Int(value));
            })
            .expect("define env.print_int");
        linker
            .func_wrap("env", "print_float", |mut caller: Caller<'_, Host>, value: f64| {
                caller.data_mut().printed.push(Printed::Float(value));
            })
            .expect("define env.print_float");

        let mut store = Store::new(&engine, Host::default());
        let instance = linker.instantiate(&mut store, &module).unwrap_or_else(|err| {
            panic!("instantiation failed (wrong import names or signatures?): {err:?}")
        });
        Vm { store, instance }
    }

    /// Call an exported function with raw wasm values.
    pub fn call(&mut self, name: &str, args: &[Val]) -> wasmtime::Result<Vec<Val>> {
        let func = self
            .instance
            .get_func(&mut self.store, name)
            .unwrap_or_else(|| panic!("the module does not export a function named `{name}`"));
        let result_count = func.ty(&self.store).results().len();
        let mut results = vec![Val::I32(0); result_count];
        func.call(&mut self.store, args, &mut results)?;
        Ok(results)
    }

    /// Call an exported `(i32...) -> i32` function. Bools and arrays travel
    /// as `i32` too.
    pub fn call_i32(&mut self, name: &str, args: &[i32]) -> i32 {
        let args: Vec<Val> = args.iter().map(|&arg| Val::I32(arg)).collect();
        let results = self
            .call(name, &args)
            .unwrap_or_else(|err| panic!("`{name}` trapped: {err:?}"));
        match results.as_slice() {
            [Val::I32(value)] => *value,
            other => panic!("`{name}` should return exactly one i32, got {other:?}"),
        }
    }

    /// Call an exported function that returns nothing.
    pub fn call_void(&mut self, name: &str, args: &[i32]) {
        let args: Vec<Val> = args.iter().map(|&arg| Val::I32(arg)).collect();
        let results = self
            .call(name, &args)
            .unwrap_or_else(|err| panic!("`{name}` trapped: {err:?}"));
        assert!(results.is_empty(), "`{name}` should return nothing, got {results:?}");
    }

    /// Call an exported `(f64...) -> f64` function.
    pub fn call_f64(&mut self, name: &str, args: &[f64]) -> f64 {
        let args: Vec<Val> = args.iter().map(|&arg| Val::F64(arg.to_bits())).collect();
        let results = self
            .call(name, &args)
            .unwrap_or_else(|err| panic!("`{name}` trapped: {err:?}"));
        match results.as_slice() {
            [Val::F64(bits)] => f64::from_bits(*bits),
            other => panic!("`{name}` should return exactly one f64, got {other:?}"),
        }
    }

    /// Call `name` expecting it to trap; returns the trap code.
    pub fn call_expecting_trap(&mut self, name: &str, args: &[i32]) -> Trap {
        let args: Vec<Val> = args.iter().map(|&arg| Val::I32(arg)).collect();
        match self.call(name, &args) {
            Ok(results) => panic!("`{name}` should have trapped but returned {results:?}"),
            Err(err) => *err
                .downcast_ref::<Trap>()
                .unwrap_or_else(|| panic!("`{name}` failed, but not with a wasm trap: {err:?}")),
        }
    }

    /// Everything printed so far.
    pub fn printed(&self) -> &[Printed] {
        &self.store.data().printed
    }

    /// The integers printed so far; panics if a float was printed.
    pub fn printed_ints(&self) -> Vec<i32> {
        self.printed()
            .iter()
            .map(|printed| match printed {
                Printed::Int(value) => *value,
                Printed::Float(value) => panic!("expected only ints, but {value} was printed"),
            })
            .collect()
    }

    /// The floats printed so far; panics if an int was printed.
    pub fn printed_floats(&self) -> Vec<f64> {
        self.printed()
            .iter()
            .map(|printed| match printed {
                Printed::Float(value) => *value,
                Printed::Int(value) => panic!("expected only floats, but {value} was printed"),
            })
            .collect()
    }

    fn memory(&mut self) -> wasmtime::Memory {
        self.instance
            .get_memory(&mut self.store, "memory")
            .expect("the module must export its linear memory as `memory`")
    }

    /// Size of linear memory in bytes.
    pub fn memory_size(&mut self) -> usize {
        let memory = self.memory();
        memory.data_size(&self.store)
    }

    /// Read the little-endian i32 at byte address `addr`.
    pub fn read_i32(&mut self, addr: usize) -> i32 {
        let memory = self.memory();
        let data = memory.data(&self.store);
        let bytes: [u8; 4] = data
            .get(addr..addr + 4)
            .unwrap_or_else(|| panic!("address {addr} is outside linear memory"))
            .try_into()
            .unwrap();
        i32::from_le_bytes(bytes)
    }

    /// Write a little-endian i32 at byte address `addr`.
    pub fn write_i32(&mut self, addr: usize, value: i32) {
        let memory = self.memory();
        memory
            .write(&mut self.store, addr, &value.to_le_bytes())
            .unwrap_or_else(|_| panic!("address {addr} is outside linear memory"));
    }

    /// Decode the `int[]` whose header is at `ptr`: the
    /// length at `ptr`, the elements from `ptr + 4`.
    pub fn read_array(&mut self, ptr: i32) -> Vec<i32> {
        let ptr = ptr as usize;
        let len = self.read_i32(ptr);
        assert!((0..=16_000).contains(&len), "implausible array length {len} at address {ptr}");
        (0..len as usize).map(|i| self.read_i32(ptr + 4 + 4 * i)).collect()
    }

    /// Allocate an `int[]` through the module's exported `alloc` and fill it
    /// from the host side. Returns the array's address.
    pub fn new_array(&mut self, values: &[i32]) -> i32 {
        let ptr = self.call_i32("alloc", &[values.len() as i32]);
        for (i, value) in values.iter().enumerate() {
            self.write_i32(ptr as usize + 4 + 4 * i, *value);
        }
        ptr
    }
}

/// Compile and run `main`, which must return an `int`.
pub fn run_int(source: &str) -> i32 {
    Vm::from_source(source).call_i32("main", &[])
}

/// Compile and run a `main` that returns nothing; gives back the VM so the
/// test can look at what was printed and at memory.
pub fn run(source: &str) -> Vm {
    let mut vm = Vm::from_source(source);
    vm.call_void("main", &[]);
    vm
}
