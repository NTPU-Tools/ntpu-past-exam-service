-- PostgreSQL schema at Alembic head 4d7c2a91b6e3.
-- Generated from SQLAlchemy metadata (the way production PostgreSQL was created) via
-- contract-tests/reference/create_schema.py, then: pg_dump --schema-only --no-owner --no-privileges.
-- This file is documentation/baseline only; Alembic remains the schema owner (see 05-data-model.md).

CREATE TABLE public.alembic_version (
    version_num character varying(32) NOT NULL
);

CREATE TABLE public.bulletins (
    title character varying(256),
    content text,
    department_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.comment_likes (
    comment_id character varying(256),
    user_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.courses (
    name character varying(256),
    category character varying(256),
    department_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.departments (
    key character varying(256),
    name character varying(256),
    is_public boolean,
    sort_order integer DEFAULT 999 NOT NULL,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.posts (
    title character varying(256),
    content text,
    owner_id character varying(256),
    course_id character varying(256),
    is_migrate boolean,
    is_anonymous boolean,
    status character varying(256),
    department_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.posts_files (
    url text,
    post_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.thread_comments (
    thread_id character varying(256),
    parent_comment_id character varying(256),
    content text,
    owner_id character varying(256),
    is_anonymous boolean,
    like_count integer,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.thread_likes (
    thread_id character varying(256),
    user_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.threads (
    title character varying(256),
    content text,
    image_url character varying(1024),
    owner_id character varying(256),
    course_id character varying(256),
    is_anonymous boolean,
    like_count integer,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.user_preferences (
    user_id character varying(256) NOT NULL,
    show_empty_courses boolean NOT NULL,
    default_is_anonymous boolean NOT NULL,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.users (
    email character varying(256),
    username character varying(256),
    readable_name character varying(256),
    note text,
    hashed_password character varying(256),
    school_department character varying(256),
    is_super_user boolean,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

CREATE TABLE public.users_departments (
    user_id character varying(256),
    department_id character varying(256),
    status character varying(256) DEFAULT 'PENDING'::character varying,
    is_department_admin boolean,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);

ALTER TABLE ONLY public.alembic_version
    ADD CONSTRAINT alembic_version_pkc PRIMARY KEY (version_num);

ALTER TABLE ONLY public.bulletins
    ADD CONSTRAINT bulletins_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT comment_likes_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.courses
    ADD CONSTRAINT courses_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.departments
    ADD CONSTRAINT departments_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.posts_files
    ADD CONSTRAINT posts_files_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT thread_comments_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT thread_likes_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT threads_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT unique_comment_like UNIQUE (comment_id, user_id);

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT unique_thread_like UNIQUE (thread_id, user_id);

ALTER TABLE ONLY public.user_preferences
    ADD CONSTRAINT user_preferences_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.users_departments
    ADD CONSTRAINT users_departments_pkey PRIMARY KEY (id);

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);

CREATE INDEX ix_bulletins_department_id ON public.bulletins USING btree (department_id);

CREATE INDEX ix_comment_likes_comment_id ON public.comment_likes USING btree (comment_id);

CREATE INDEX ix_comment_likes_user_id ON public.comment_likes USING btree (user_id);

CREATE INDEX ix_courses_department_id ON public.courses USING btree (department_id);

CREATE INDEX ix_departments_is_public ON public.departments USING btree (is_public);

CREATE INDEX ix_posts_course_id ON public.posts USING btree (course_id);

CREATE INDEX ix_posts_department_id ON public.posts USING btree (department_id);

CREATE INDEX ix_posts_files_post_id ON public.posts_files USING btree (post_id);

CREATE INDEX ix_posts_owner_id ON public.posts USING btree (owner_id);

CREATE INDEX ix_posts_status ON public.posts USING btree (status);

CREATE INDEX ix_thread_comments_owner_id ON public.thread_comments USING btree (owner_id);

CREATE INDEX ix_thread_comments_parent_comment_id ON public.thread_comments USING btree (parent_comment_id);

CREATE INDEX ix_thread_comments_thread_id ON public.thread_comments USING btree (thread_id);

CREATE INDEX ix_thread_likes_thread_id ON public.thread_likes USING btree (thread_id);

CREATE INDEX ix_thread_likes_user_id ON public.thread_likes USING btree (user_id);

CREATE INDEX ix_threads_course_id ON public.threads USING btree (course_id);

CREATE INDEX ix_threads_owner_id ON public.threads USING btree (owner_id);

CREATE UNIQUE INDEX ix_user_preferences_user_id ON public.user_preferences USING btree (user_id);

CREATE INDEX ix_users_departments_department_id ON public.users_departments USING btree (department_id);

CREATE INDEX ix_users_departments_is_department_admin ON public.users_departments USING btree (is_department_admin);

CREATE INDEX ix_users_departments_status ON public.users_departments USING btree (status);

CREATE INDEX ix_users_departments_user_id ON public.users_departments USING btree (user_id);

CREATE UNIQUE INDEX ix_users_username ON public.users USING btree (username);

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT comment_likes_comment_id_fkey FOREIGN KEY (comment_id) REFERENCES public.thread_comments(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT comment_likes_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id);

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT thread_comments_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.users(id);

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT thread_comments_parent_comment_id_fkey FOREIGN KEY (parent_comment_id) REFERENCES public.thread_comments(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT thread_comments_thread_id_fkey FOREIGN KEY (thread_id) REFERENCES public.threads(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT thread_likes_thread_id_fkey FOREIGN KEY (thread_id) REFERENCES public.threads(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT thread_likes_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id);

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT threads_course_id_fkey FOREIGN KEY (course_id) REFERENCES public.courses(id);

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT threads_owner_id_fkey FOREIGN KEY (owner_id) REFERENCES public.users(id);

ALTER TABLE ONLY public.user_preferences
    ADD CONSTRAINT user_preferences_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.users_departments
    ADD CONSTRAINT users_departments_department_id_fkey FOREIGN KEY (department_id) REFERENCES public.departments(id) ON DELETE CASCADE;

ALTER TABLE ONLY public.users_departments
    ADD CONSTRAINT users_departments_user_id_fkey FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;

