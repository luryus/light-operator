use kube::CustomResourceExt;
use light_operator::kubernetes::crd::Light;

fn main() -> Result<(), serde_yaml::Error> {
    let crd = Light::crd();
    println!("{}", serde_yaml::to_string(&crd)?);
    Ok(())
}
