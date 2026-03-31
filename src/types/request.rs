use axum::{
    body::Body,
    http::Request as HttpRequest,
};
use log::debug;
use pyo3::types::{PyBytes, PyDict, PyString};
use pyo3::{exceptions::PyValueError, prelude::*};
use serde_json::Value;
use std::collections::HashMap;

use crate::types::{check_body_type, get_body_from_pyobject, Url};

use super::{headers::Headers, identity::Identity, multimap::QueryParams};

#[derive(Default, Debug, Clone, FromPyObject)]
pub struct Request {
    pub query_params: QueryParams,
    pub headers: Headers,
    pub method: String,
    pub path_params: HashMap<String, String>,
    // https://pyo3.rs/v0.19.2/function.html?highlight=from_py_#per-argument-options
    #[pyo3(from_py_with = "get_body_from_pyobject")]
    pub body: Vec<u8>,
    pub url: Url,
    pub ip_addr: Option<String>,
    pub identity: Option<Identity>,
    pub form_data: Option<HashMap<String, String>>,
    pub files: Option<HashMap<String, Vec<u8>>>,
}

impl ToPyObject for Request {
    fn to_object(&self, py: Python) -> PyObject {
        let query_params = self.query_params.clone();
        let headers: Py<Headers> = self.headers.clone().into_py(py).extract(py).unwrap();
        let path_params = self.path_params.clone().into_py(py).extract(py).unwrap();
        let body = match String::from_utf8(self.body.clone()) {
            Ok(s) => s.into_py(py),
            Err(_) => self.body.clone().into_py(py),
        };
        let form_data: Py<PyDict> = match &self.form_data {
            Some(data) => {
                let dict = PyDict::new(py);
                for (key, value) in data.iter() {
                    dict.set_item(key, value).unwrap();
                }
                dict.into_py(py)
            }
            None => PyDict::new(py).into_py(py),
        };

        let files: Py<PyDict> = match &self.files {
            Some(data) => {
                let dict = PyDict::new(py);
                for (key, value) in data.iter() {
                    let bytes = PyBytes::new(py, value);
                    dict.set_item(key, bytes).unwrap();
                }
                dict.into_py(py)
            }
            None => PyDict::new(py).into_py(py),
        };

        let request = PyRequest {
            query_params,
            path_params,
            headers,
            body,
            method: self.method.clone(),
            url: self.url.clone(),
            ip_addr: self.ip_addr.clone(),
            identity: self.identity.clone(),
            form_data: form_data.clone(),
            files: files.clone(),
        };
        Py::new(py, request).unwrap().as_ref(py).into()
    }
}

impl Request {
    pub async fn from_axum_request(
        req: HttpRequest<Body>,
        global_headers: &Headers,
    ) -> Self {
        let mut query_params: QueryParams = QueryParams::new();
        let form_data: HashMap<String, String> = HashMap::new();
        let files = HashMap::new();

        if let Some(query) = req.uri().query() {
            if !query.is_empty() {
                let split = query.split('&');
                for s in split {
                    let path_params = s.split_once('=').unwrap_or((s, ""));
                    let key = path_params.0.to_string();
                    let value = path_params.1.to_string();

                    query_params.set(key, value);
                }
            }
        }

        let mut headers = Headers::from_http_headers(req.headers());
        debug!("Global headers: {:?}", global_headers);
        headers.extend(global_headers);

        let has_multipart = headers.contains(String::from("content-type"))
            && headers
                .get(String::from("content-type"))
                .is_some_and(|val| val.contains("multipart/form-data"));

        if has_multipart {
            let content_type = headers.get(String::from("content-type")).unwrap();
            debug!("Content-Type: {:?}", content_type);
        }

        // Extract everything we need BEFORE consuming the body
        let method = req.method().as_str().to_owned();
        let host = req
            .headers()
            .get("host")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("localhost")
            .to_string();

        let scheme = req
            .uri()
            .scheme()
            .map(|s| s.as_str().to_string())
            .unwrap_or_else(|| "http".to_string());

        let path = req.uri().path().to_string();
        let url = Url::new(&scheme, &host, &path);

        // Now consume the body
        let body = http_body_util::BodyExt::collect(req.into_body())
            .await
            .map(|c| c.to_bytes().to_vec())
            .unwrap_or_default();

        debug!("Request body: {:?}", body);
        debug!("Request headers: {:?}", headers);
        debug!("Request query params: {:?}", query_params);
        debug!("Request form data: {:?}", form_data);
        debug!("Request files: {:?}", files);

        let ip_addr = None; // Will be populated from ConnectInfo if needed

        Self {
            query_params,
            headers,
            method,
            path_params: HashMap::new(),
            body,
            url,
            ip_addr,
            identity: None,
            form_data: Some(form_data),
            files: Some(files),
        }
    }
}

#[pyclass(name = "Request")]
#[derive(Clone)]
pub struct PyRequest {
    #[pyo3(get, set)]
    pub query_params: QueryParams,
    #[pyo3(get, set)]
    pub headers: Py<Headers>,
    #[pyo3(get, set)]
    pub path_params: Py<PyDict>,
    #[pyo3(get, set)]
    pub identity: Option<Identity>,
    #[pyo3(get)]
    pub body: Py<PyAny>,
    #[pyo3(get)]
    pub method: String,
    #[pyo3(get)]
    pub url: Url,
    #[pyo3(get)]
    pub ip_addr: Option<String>,
    #[pyo3(get, set)]
    pub form_data: Py<PyDict>,
    #[pyo3(get, set)]
    pub files: Py<PyDict>,
}

#[pymethods]
impl PyRequest {
    #[new]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        query_params: QueryParams,
        headers: Py<Headers>,
        path_params: Py<PyDict>,
        body: Py<PyAny>,
        method: String,
        url: Url,
        form_data: Py<PyDict>,
        files: Py<PyDict>,
        identity: Option<Identity>,
        ip_addr: Option<String>,
    ) -> Self {
        Self {
            query_params,
            headers,
            path_params,
            identity,
            body,
            method,
            url,
            form_data,
            files,
            ip_addr,
        }
    }

    #[setter]
    pub fn set_body(&mut self, py: Python, body: Py<PyAny>) -> PyResult<()> {
        check_body_type(py, &body)?;
        self.body = body;
        Ok(())
    }

    pub fn json(&self, py: Python) -> PyResult<PyObject> {
        match self.body.as_ref(py).downcast::<PyString>() {
            Ok(python_string) => match serde_json::from_str(python_string.extract()?) {
                Ok(Value::Object(map)) => {
                    let dict = PyDict::new(py);

                    for (key, value) in map.iter() {
                        let py_key = key.to_string().into_py(py);
                        let py_value = match value {
                            Value::String(s) => s.as_str().into_py(py),
                            _ => value.to_string().into_py(py),
                        };

                        dict.set_item(py_key, py_value)?;
                    }

                    Ok(dict.into_py(py))
                }
                _ => Err(PyValueError::new_err("Invalid JSON object")),
            },
            Err(e) => Err(e.into()),
        }
    }
}
