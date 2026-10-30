// moon: The build system and package manager for MoonBit.
// Copyright (C) 2026 International Digital Economy Academy
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND,
// either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// For inquiries, you can contact us via e-mail at jichuruanjian@idea.edu.cn.

use std::{
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use wee8::{Error, Store};

// A broken interrupt must fail CI instead of leaving a guest loop running forever.
#[test]
fn termination_and_disposal() {
    const CHILD: &str = "WEE8_TEST_TERMINATION_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "termination_and_disposal", "--nocapture"])
            .env(CHILD, "1")
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "termination child failed: {status}");
                return;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("V8 termination failed to stop a guest within 30 seconds");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    for mode in ["dynamic", "typed", "start"] {
        let store = Store::new(()).unwrap();
        let handle = store.isolate_handle().unwrap();
        let (entered, receive) = mpsc::channel();
        let host = store
            .host_func(&[], &[], move |_, _| {
                entered.send(()).unwrap();
                Ok(vec![])
            })
            .unwrap();
        let start = if mode == "start" { "(start $run)" } else { "" };
        let module = store
            .compile(
                &wat::parse_str(format!(
                    r#"(module
          (import "host" "entered" (func $entered))
          (func $run (export "run")
            block $caught
              try_table (catch_all $caught)
                call $entered
                loop $spin br $spin end
              end
            end
            unreachable)
          (func (export "ok") (result i32) i32.const 42)
          {start})"#
                ))
                .unwrap(),
            )
            .unwrap();
        let remote = handle.clone();
        let interrupt = thread::spawn(move || {
            receive.recv_timeout(Duration::from_secs(10)).unwrap();
            assert!(remote.terminate_execution());
        });
        if mode == "start" {
            assert!(matches!(
                module.instantiate(&[&host]),
                Err(Error::Terminated)
            ));
        } else {
            let instance = module.instantiate(&[&host]).unwrap();
            let run = instance.get_func("run").unwrap();
            let result = if mode == "typed" {
                run.typed::<(), ()>().unwrap().call(())
            } else {
                run.call(&[]).map(|_| ())
            };
            assert!(
                matches!(result, Err(Error::Terminated)),
                "{mode}: {result:?}"
            );
            assert!(store.take_exception().is_none());
            interrupt.join().unwrap();
            assert!(handle.cancel_terminate_execution());
            assert_eq!(
                instance
                    .get_func("ok")
                    .unwrap()
                    .typed::<(), i32>()
                    .unwrap()
                    .call(())
                    .unwrap(),
                42
            );
            continue;
        }
        interrupt.join().unwrap();
        assert!(handle.cancel_terminate_execution());
    }
    // An idle request must survive allocating C API housekeeping. In particular,
    // instantiation must not consume it before the start function enters V8.
    for start in [false, true] {
        let store = Store::new(()).unwrap();
        let handle = store.isolate_handle().unwrap();
        assert!(handle.terminate_execution());
        let host = store.host_func(&[], &[], |_, _| Ok(vec![])).unwrap();
        let start_section = if start { "(start $run)" } else { "" };
        let module = store
            .compile(
                &wat::parse_str(format!(
                    r#"(module
          (import "host" "unused" (func))
          (func $run (export "run") loop $spin br $spin end)
          {start_section})"#
                ))
                .unwrap(),
            )
            .unwrap();
        if start {
            assert!(matches!(
                module.instantiate(&[&host]),
                Err(Error::Terminated)
            ));
        } else {
            let instance = module.instantiate(&[&host]).unwrap();
            let run = instance.get_func("run").unwrap().typed::<(), ()>().unwrap();
            assert!(matches!(run.call(()), Err(Error::Terminated)));
        }
        assert!(handle.cancel_terminate_execution());
    }
    // V8 cannot preempt a host operation. The embedder releases its wait, then
    // the termination request is observed when control returns to Wasm.
    {
        let store = Store::new(()).unwrap();
        let handle = store.isolate_handle().unwrap();
        let (entered, receive) = mpsc::channel();
        let (release, waiting) = mpsc::channel();
        let host = store
            .host_func(&[], &[], move |_, _| {
                entered.send(()).unwrap();
                waiting.recv_timeout(Duration::from_secs(10)).unwrap();
                Ok(vec![])
            })
            .unwrap();
        let module = store
            .compile(
                &wat::parse_str(
                    r#"(module
              (import "host" "wait" (func $wait))
              (func (export "run") call $wait loop $spin br $spin end))"#,
                )
                .unwrap(),
            )
            .unwrap();
        let instance = module.instantiate(&[&host]).unwrap();
        let interrupt = thread::spawn(move || {
            receive.recv_timeout(Duration::from_secs(10)).unwrap();
            assert!(handle.terminate_execution());
            release.send(()).unwrap();
        });
        assert!(matches!(
            instance.get_func("run").unwrap().call(&[]),
            Err(Error::Terminated)
        ));
        interrupt.join().unwrap();
    }
    // Cancellation before execution, and an unrelated store, must remain usable.
    let store = Store::new(()).unwrap();
    let handle = store.isolate_handle().unwrap();
    assert!(handle.terminate_execution());
    assert!(handle.cancel_terminate_execution());
    let module = store
        .compile(&wat::parse_str("(module (func (export \"ok\")))").unwrap())
        .unwrap();
    module
        .instantiate(&[])
        .unwrap()
        .get_func("ok")
        .unwrap()
        .typed::<(), ()>()
        .unwrap()
        .call(())
        .unwrap();
    drop(module);
    drop(store);
    assert!(!handle.terminate_execution());
    assert!(!handle.cancel_terminate_execution());

    // Concurrent requests race store disposal, including handles retained afterward.
    for _ in 0..100 {
        let store = Store::new(()).unwrap();
        let handle = store.isolate_handle().unwrap();
        let remote = handle.clone();
        let worker = thread::spawn(move || {
            for _ in 0..100 {
                remote.terminate_execution();
                remote.cancel_terminate_execution();
            }
        });
        drop(store);
        worker.join().unwrap();
        assert!(!handle.terminate_execution());
        assert!(!handle.cancel_terminate_execution());
    }
}

#[test]
fn requests_are_isolate_local_and_trap_messages_cannot_forge_termination() {
    let interrupted = Store::new(()).unwrap();
    let control = interrupted.isolate_handle().unwrap();
    assert!(control.terminate_execution());

    let other = Store::new(()).unwrap();
    let host = other
        .host_func(&[], &[], |_, _| Err("execution terminated".into()))
        .unwrap();
    let module = other
        .compile(
            &wat::parse_str(
                r#"(module
      (import "host" "fail" (func $fail))
      (func (export "ok") (result i32) i32.const 42)
      (func (export "fail") call $fail))"#,
            )
            .unwrap(),
        )
        .unwrap();
    let instance = module.instantiate(&[&host]).unwrap();
    assert_eq!(
        instance.get_func("ok").unwrap().call(&[]).unwrap(),
        [wee8::Value::I32(42)]
    );
    assert!(matches!(
        instance.get_func("fail").unwrap().call(&[]),
        Err(Error::Host(_))
    ));
    assert!(control.cancel_terminate_execution());
}

#[test]
fn handles_do_not_retain_host_state() {
    use std::{cell::Cell, rc::Rc};
    struct Finalized(Rc<Cell<bool>>);
    impl Drop for Finalized {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let store = Store::new(Finalized(dropped.clone())).unwrap();
    let handle = store.isolate_handle().unwrap();
    let callback = store.host_func(&[], &[], |_, _| Ok(vec![])).unwrap();
    drop(callback);
    drop(store);
    assert!(dropped.get());
    assert!(!handle.terminate_execution());
    assert!(!handle.clone().cancel_terminate_execution());
}
