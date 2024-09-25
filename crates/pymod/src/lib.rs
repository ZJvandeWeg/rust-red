use edgelink_core::runtime::model::{ElementId, Msg};
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pyo3::{prelude::*, wrap_pyfunction};
use serde::Deserialize;
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
fn run_flows_once<'a>(
    py: Python<'a>,
    _expected_msgs: usize,
    _timeout: f64,
    py_json: &'a PyAny,
    msgs_json: &'a PyAny,
    app_cfg: &'a PyAny,
) -> PyResult<&'a PyAny> {
    let flows_json = py_object_to_json_value(py, py_json)?;
    let msgs_to_inject = {
        let json_msgs = py_object_to_json_value(py, msgs_json)?;
        Vec::<(ElementId, Msg)>::deserialize(json_msgs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?
    };
    let app_cfg = {
        if !app_cfg.is_none() {
            let app_cfg_json = py_object_to_json_value(py, app_cfg)?;
            let config = config::Config::try_from(&app_cfg_json)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;
            Some(config)
        } else {
            None
        }
    };

    pyo3_asyncio::tokio::future_into_py(py, async move {
        let registry = edgelink_core::runtime::registry::RegistryBuilder::default()
            .build()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;

        let engine = FlowEngine::new_with_json(registry, &flows_json, app_cfg.as_ref())
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;

        let msgs = engine
            .run_once_with_inject(_expected_msgs, std::time::Duration::from_secs_f64(_timeout), msgs_to_inject)
            .await
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;

        let result_value = serde_json::to_value(&msgs)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("{}", e)))?;

        Python::with_gil(|py| Ok(json_value_to_py_object(py, &result_value)?.to_object(py)))
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
    } else if let Ok(boolean) = obj.downcast::<PyBool>() {
        Ok(Value::Bool(boolean.extract::<bool>()?))
    } else if let Ok(float) = obj.downcast::<PyFloat>() {
        let num = float.extract::<f64>()?;
        Ok(serde_json::json!(num))
    } else if let Ok(int) = obj.downcast::<PyInt>() {
        let num = int.extract::<i64>()?;
        Ok(serde_json::json!(num))
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
