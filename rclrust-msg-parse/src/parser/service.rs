use std::{fs, path::Path};

use anyhow::{Context, Result};

use super::{error::RclMsgError, message::parse_message_string};
use crate::types::Service;

const SERVICE_REQUEST_SUFFIX: &str = "_Request";
const SERVICE_RESPONSE_SUFFIX: &str = "_Response";

pub fn parse_service_file<P>(pkg_name: &str, interface_file: P) -> Result<Service>
where
    P: AsRef<Path>,
{
    let interface_file = interface_file.as_ref();
    let service_string = fs::read_to_string(interface_file)?.replace("\r\n", "\n");

    parse_service_string(
        pkg_name,
        interface_file.file_stem().unwrap().to_str().unwrap(),
        &service_string,
    )
    .with_context(|| format!("Parse file error: {}", interface_file.display()))
}

fn parse_service_string(pkg_name: &str, srv_name: &str, service_string: &str) -> Result<Service> {
    let (block1, block2) = service_string.split_once("---\n").ok_or_else(|| {
        RclMsgError::InvalidServiceSpecification(format!(
            "Expect one '---' seperator in {}/{} service definition",
            pkg_name, srv_name,
        ))
    })?;

    Ok(Service {
        package: pkg_name.into(),
        name: srv_name.into(),
        request: parse_message_string(
            pkg_name,
            &format!("{}{}", srv_name, SERVICE_REQUEST_SUFFIX),
            block1,
        )?,
        response: parse_message_string(
            pkg_name,
            &format!("{}{}", srv_name, SERVICE_RESPONSE_SUFFIX),
            block2,
        )?,
    })
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use super::*;

    fn parse_srv_def(srv_name: &str) -> Result<Service> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join(format!("test_msgs/srv/{}.srv", srv_name));
        parse_service_file("test_msgs", path)
    }

    #[test]
    fn parse_arrays() -> Result<()> {
        let result = parse_srv_def("Arrays")?;
        assert_eq!(result.package, "test_msgs".to_string());
        assert_eq!(result.name, "Arrays".to_string());
        assert_eq!(result.request.name, "Arrays_Request".to_string());
        assert_eq!(result.response.name, "Arrays_Response".to_string());
        Ok(())
    }

    #[test]
    fn parse_basic_types() -> Result<()> {
        let _result = parse_srv_def("BasicTypes")?;
        Ok(())
    }

    #[test]
    fn parse_empty() -> Result<()> {
        let _result = parse_srv_def("Empty")?;
        Ok(())
    }
}
