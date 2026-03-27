# Sveltekit SSR (Server Side Rendering)

This example shows how to use Lambda Web Adapter to run a [server side rendered Sveltekit](https://svelte.dev/tutorial/kit/ssr) application on the managed nodejs runtime. 

### How does it work?

Add the Lambda Web Adapter layer to the function and configure the wrapper script. 

1. attach Lambda Adapter layer to your function. This layer contains the Lambda Adapter binary and a wrapper script. 
    1. x86_64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerX86:27`
    2. arm64: `arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:27`
2. configure Lambda environment variable `AWS_LAMBDA_EXEC_WRAPPER` to `/opt/bootstrap`. This is a wrapper script included in the layer.
3. set function handler to a startup command: `run.sh`. The wrapper script will execute this command to boot up your application. 

To get more information of Wrapper script, please read Lambda documentation [here](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper). 

### Create and configure SvelteKit SSR app

\* *this example was created from the steps in this section. repeating them is not required*

1. `npx sv create app`
    1. select `SvelteKit minimal` option
    1. select `Yes, using Typescript syntax` option
    1. repeatedly select enter to complete sveltekit install with default options

1. `cd app` to switch current working directory to newly created `app` directory:
    1. `npm install --save-dev @sveltejs/adapter-node` to install sveltekit [node adapter](https://svelte.dev/docs/kit/adapter-node)
    1. `npm uninstall @sveltejs/adapter-auto` to remove unused auto adapter
    1. replace `import adapter from '@sveltejs/adapter-auto';` with `import adapter from '@sveltejs/adapter-node';` in `svelte.config.js`
    1. add a `run.sh` [wrapper](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-modify.html#runtime-wrapper) script:
    ```sh
    cat << EOF > ./run.sh
    #!/bin/bash

    node index.js
    EOF
    ```

### Build and deploy SSR SvelteKit on Lambda

Run the following commands to build and deploy the application to lambda. 

```bash
sam build --use-container
sam deploy --guided
```
When the deployment completes, take note of the SvelteKitSsrFunctionUrlEndpoint output value. This is the function URL. 

### Verify it works

Open function's URL in a browser to display the "Welcome to SvelteKit" page.
