# Troubleshooting

This page contains common issues and their solutions when working with OpenSleep.

## Device Not Responding

**Issue:** Device not responding in `manager.rs`

**Solution:** This can happen if the frank service wasn't disabled. Make sure to disable the frank service before attempting to communicate with the device.

## SleepEight App Crashes After Disabling Services

**Issue:** The SleepEight app crashes after disabling the services.

**Solution:** Clearing cache and data doesn't work in this case. The following steps seem to resolve the app crashing:

1. Fully uninstall the SleepEight app
2. Reinstall the app
3. Exit out of setup as soon as the pod is paired, leaving no pod on your account

**Note:**  To avoid having this happen again, remove the pod from account prior to factoring resetting it.

## Public Key Errors

**Issue:** Encountering public key errors when trying to SSH into the device.

**Solution:** You have a short window to SSH in and disable the services before the update service updates the pod. Once updated, your SSH keys will be dropped. Act quickly after initial setup to disable the update service.

**Note:** If your able to connect and then the ssh fingerprint changes, resulting in you being unable to connect, that is a sign of an update taken place, you'll have to start over.

## Factory Reset vs Live Partition Update

**Question:** Do I need to factory reset, or can I just update the live partition?

**Answer:** (Speculation) It seems the pod's security does some checking where the live partition authorized keys won't work if they don't match a signature from the factory reset. This impacts going back to the base image. A factory reset is recommended to ensure proper key authentication.

## Reverting to Base Image

**Question:** How do I go back to the base image?

**Answer:** Assuming you've written back the original SD card image or are using the original SD card:

1. Place the old SD card back in the device
2. Perform a factory reset
3. **Important:** Using the card without a factory reset will run into security issues that prevent SSH from working

The factory reset is required for the authorized keys to work properly with the restored base image.
