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

use wee8::{Store, Value};

#[test]
fn saturating_conversions_execute_through_scalar_exports() {
    for (output, signedness, minimum, maximum) in [
        ("i32", "s", i32::MIN as i64, i32::MAX as i64),
        ("i32", "u", 0, u32::MAX as i64),
        ("i64", "s", i64::MIN, i64::MAX),
        ("i64", "u", 0, -1),
    ] {
        for input in ["f32", "f64"] {
            let demote = if input == "f32" { "f32.demote_f64" } else { "" };
            let extend = if output == "i32" {
                format!("i64.extend_i32_{signedness}")
            } else {
                String::new()
            };
            let source = format!(
                "(module (func (export \"run\") (param f64) (result i64)
                local.get 0 {demote} {output}.trunc_sat_{input}_{signedness} {extend}))"
            );
            let store = Store::new(()).unwrap();
            let module = store.compile(&wat::parse_str(source).unwrap()).unwrap();
            let compiled = module.serialize();
            let next_store = Store::new(()).unwrap();
            let module = next_store.module(&compiled).unwrap();
            let mut instance = module.instantiate(&[]).unwrap();
            for (value, expected) in [
                (f64::NAN, 0),
                (f64::NEG_INFINITY, minimum),
                (f64::INFINITY, maximum),
                (42.75, 42),
                (-42.75, if signedness == "s" { -42 } else { 0 }),
            ] {
                assert_eq!(
                    instance.call("run", &[Value::F64(value)]).unwrap(),
                    [Value::I64(expected)],
                    "{output}.trunc_sat_{input}_{signedness}({value})"
                );
            }
        }
    }
}
