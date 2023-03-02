# SlimeVR Reset Time App

Shows time since reset, integrates into OBS, logs reset times into `resettime.log`.

Requires SlimeVR v0.7.0+.

![image](https://user-images.githubusercontent.com/114709761/221412007-7fc0f49a-5c40-4683-a25d-e3c13bd2b444.png)

## Config

Generates `appconfig.toml` on first launch, where you can set paths to SlimeVR log and config and OBS settings:

```toml
[slimevr]
dir = 'C:\Program Files (x86)\SlimeVR Server\'
log = "log_last_0.log"
vrconfig = "vrconfig.yml"

[obs]
host = "localhost"
port = 4455
password = ""
# Text field for reset time
text_time = "slimetime"
text_time_format = "Reset #{num} {time}"
# Text field for active settings
text_config = "slimeconfig"
```

## License

See LICENSE-APACHE and LICENSE-MIT for details.
