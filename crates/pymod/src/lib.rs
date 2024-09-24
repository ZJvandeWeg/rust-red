use std::sync::Arc;

use pyo3::types::{PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pyo3::{prelude::*, wrap_pyfunction};
use serde_json::{Map, Value};

use edgelink_core::runtime::engine::FlowEngine;

#[pymodule]
fn edgelink_pymod(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(rust_sleep, m)?)?;
    m.add_function(wrap_pyfunction!(run_flows_once, m)?)?;
    Ok(())
}

#[pyfunction]
fn rust_sleep(py: Python) -> PyResult<&PyAny> {
    pyo3_asyncio::tokio::future_into_py(py, async {
        eprintln!("Sleeping in Rust!");
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        Ok(())
    })
}

#[pyfunction]
fn run_flows_once<'a>(py: Python<'a>, _expected_msgs: usize, _timeout: f64, py_json: &'a PyAny) -> PyResult<&'a PyAny> {
    let flows_json = Arc::new(py_object_to_json_value(py, py_json)?);

    pyo3_asyncio::tokio::future_into_py_with_locals(py, pyo3_asyncio::tokio::get_current_locals(py)?, async move {
        let registry = edgelink_core::runtime::registry::RegistryBuilder::default()
            .build()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;

        let engine = FlowEngine::new_with_json(registry, &flows_json, None)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;

        /*
        let msgs = engine
            .run_once(1, std::time::Duration::from_millis(200))
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;

        let result_value = serde_json::to_value(&msgs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;
        tokio::time::sleep(std::time::Duration::from_secs_f64(0.1)).await;
        */
        let result_value = serde_json::json!({"foo":"bar", "ints":[1,2,3,4,5]});
        let res = json_value_to_py_object(py, &result_value)?;
        Python::with_gil(|py| Ok(()))
    })
}

fn py_object_to_json_value(py: Python, obj: &PyAny) -> PyResult<Value> {
    if let Ok(list) = obj.downcast::<PyList>() {
        let mut json_list = Vec::new();
        for item in list.iter() {
            json_list.push(py_object_to_json_value(py, item)?);
        }
        Ok(Value::Array(json_list))
    } else if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut json_map = Map::new();
        for (key, value) in dict.iter() {
            let key = key.extract::<String>()?;
            let value = py_object_to_json_value(py, value)?;
            json_map.insert(key, value);
        }
        Ok(Value::Object(json_map))
    } else if let Ok(tuple) = obj.downcast::<PyTuple>() {
        let mut json_list = Vec::new();
        for item in tuple.iter() {
            json_list.push(py_object_to_json_value(py, item)?);
        }
        Ok(Value::Array(json_list))
    } else if let Ok(float) = obj.downcast::<PyFloat>() {
        let num = float.extract::<f64>()?;
        Ok(Value::Number(
            serde_json::Number::from_f64(num)
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid float value"))?,
        ))
    } else if let Ok(int) = obj.downcast::<PyInt>() {
        let num = int.extract::<i64>()? as f64;
        Ok(Value::Number(
            serde_json::Number::from_f64(num)
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid int value"))?,
        ))
    } else if let Ok(string) = obj.downcast::<PyString>() {
        Ok(Value::String(string.extract::<String>()?))
    } else {
        Ok(Value::Null)
    }
}

fn json_value_to_py_object(py: Python, value: &Value) -> PyResult<PyObject> {
    match value {
        Value::Null => Ok(py.None()),
        Value::Bool(b) => Ok(b.into_py(py)),
        Value::Number(n) => {
            if let Some(int) = n.as_i64() {
                Ok(int.to_object(py))
            } else if let Some(float) = n.as_f64() {
                Ok(PyFloat::new(py, float).into())
            } else {
                Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Invalid number type"))
            }
        }
        Value::String(s) => Ok(PyString::new(py, s).into()),
        Value::Array(arr) => {
            let list = PyList::empty(py);
            for item in arr {
                let py_item = json_value_to_py_object(py, item)?;
                list.append(py_item)?;
            }
            Ok(list.into())
        }
        Value::Object(obj) => {
            let dict = PyDict::new(py);
            for (key, value) in obj {
                let py_key = PyString::new(py, key);
                let py_value = json_value_to_py_object(py, value)?;
                dict.set_item(py_key, py_value)?;
            }
            Ok(dict.into())
        }
    }
}
