FROM public.ecr.aws/awsguru/php:82.2023.3.11.1 AS builder
COPY --from=composer /usr/bin/composer /usr/local/bin/composer

COPY app /var/task/app
WORKDIR /var/task/app

RUN composer install --prefer-dist --optimize-autoloader --no-dev --no-interaction

FROM public.ecr.aws/awsguru/php:82.2023.3.11.1
COPY --from=public.ecr.aws/awsguru/aws-lambda-adapter:1.0.0 /lambda-adapter /opt/extensions/lambda-adapter
COPY --from=builder /var/task /var/task

# config files
ADD nginx/conf/nginx.conf      /opt/nginx/conf/nginx.conf
ADD php/php.ini                /opt/php/php.ini
ADD php/etc/php-fpm.conf       /opt/php/etc/php-fpm.conf
ADD php/php.d/extensions.ini   /opt/php/php.d/extensions.ini

COPY bootstrap /opt/bootstrap

ENTRYPOINT /opt/bootstrap
