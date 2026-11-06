-- Baseline schema at the previous Alembic head (4d7c2a91b6e3).
-- Generated with pg_dump --schema-only; apply through `past-exam-service migrate`.

--
-- PostgreSQL database dump
--


-- Dumped from database version 17.11
-- Dumped by pg_dump version 17.11

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: public; Type: SCHEMA; Schema: -; Owner: -
--



--
-- Name: SCHEMA public; Type: COMMENT; Schema: -; Owner: -
--



SET default_tablespace = '';

SET default_table_access_method = heap;

--


--
-- Name: bulletins; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.bulletins (
    title character varying(256),
    content text,
    department_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--
-- Name: comment_likes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.comment_likes (
    comment_id character varying(256),
    user_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--
-- Name: courses; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.courses (
    name character varying(256),
    category character varying(256),
    department_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--
-- Name: departments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.departments (
    key character varying(256),
    name character varying(256),
    is_public boolean,
    sort_order integer DEFAULT 999 NOT NULL,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--
-- Name: posts; Type: TABLE; Schema: public; Owner: -
--

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


--
-- Name: posts_files; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.posts_files (
    url text,
    post_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--
-- Name: thread_comments; Type: TABLE; Schema: public; Owner: -
--

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


--
-- Name: thread_likes; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.thread_likes (
    thread_id character varying(256),
    user_id character varying(256),
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--
-- Name: threads; Type: TABLE; Schema: public; Owner: -
--

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


--
-- Name: user_preferences; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.user_preferences (
    user_id character varying(256) NOT NULL,
    show_empty_courses boolean NOT NULL,
    default_is_anonymous boolean NOT NULL,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--
-- Name: users; Type: TABLE; Schema: public; Owner: -
--

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


--
-- Name: users_departments; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.users_departments (
    user_id character varying(256),
    department_id character varying(256),
    status character varying(256) DEFAULT 'PENDING'::character varying,
    is_department_admin boolean,
    id character varying(256) NOT NULL,
    create_time timestamp with time zone DEFAULT now(),
    updated_time timestamp with time zone
);


--


--
-- Name: bulletins bulletins_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.bulletins
    ADD CONSTRAINT bulletins_pkey PRIMARY KEY (id);


--
-- Name: comment_likes comment_likes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT comment_likes_pkey PRIMARY KEY (id);


--
-- Name: courses courses_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.courses
    ADD CONSTRAINT courses_pkey PRIMARY KEY (id);


--
-- Name: departments departments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.departments
    ADD CONSTRAINT departments_pkey PRIMARY KEY (id);


--
-- Name: posts_files posts_files_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts_files
    ADD CONSTRAINT posts_files_pkey PRIMARY KEY (id);


--
-- Name: posts posts_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.posts
    ADD CONSTRAINT posts_pkey PRIMARY KEY (id);


--
-- Name: thread_comments thread_comments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT thread_comments_pkey PRIMARY KEY (id);


--
-- Name: thread_likes thread_likes_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT thread_likes_pkey PRIMARY KEY (id);


--
-- Name: threads threads_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT threads_pkey PRIMARY KEY (id);


--
-- Name: comment_likes unique_comment_like; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT unique_comment_like UNIQUE (comment_id, user_id);


--
-- Name: thread_likes unique_thread_like; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT unique_thread_like UNIQUE (thread_id, user_id);


--
-- Name: user_preferences user_preferences_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_preferences
    ADD CONSTRAINT user_preferences_pkey PRIMARY KEY (id);


--
-- Name: users_departments users_departments_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users_departments
    ADD CONSTRAINT users_departments_pkey PRIMARY KEY (id);


--
-- Name: users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: ix_bulletins_department_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_bulletins_department_id ON public.bulletins USING btree (department_id);


--
-- Name: ix_comment_likes_comment_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_comment_likes_comment_id ON public.comment_likes USING btree (comment_id);


--
-- Name: ix_comment_likes_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_comment_likes_user_id ON public.comment_likes USING btree (user_id);


--
-- Name: ix_courses_department_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_courses_department_id ON public.courses USING btree (department_id);


--
-- Name: ix_departments_is_public; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_departments_is_public ON public.departments USING btree (is_public);


--
-- Name: ix_posts_course_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_posts_course_id ON public.posts USING btree (course_id);


--
-- Name: ix_posts_department_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_posts_department_id ON public.posts USING btree (department_id);


--
-- Name: ix_posts_files_post_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_posts_files_post_id ON public.posts_files USING btree (post_id);


--
-- Name: ix_posts_owner_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_posts_owner_id ON public.posts USING btree (owner_id);


--
-- Name: ix_posts_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_posts_status ON public.posts USING btree (status);


--
-- Name: ix_thread_comments_owner_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_thread_comments_owner_id ON public.thread_comments USING btree (owner_id);


--
-- Name: ix_thread_comments_parent_comment_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_thread_comments_parent_comment_id ON public.thread_comments USING btree (parent_comment_id);


--
-- Name: ix_thread_comments_thread_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_thread_comments_thread_id ON public.thread_comments USING btree (thread_id);


--
-- Name: ix_thread_likes_thread_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_thread_likes_thread_id ON public.thread_likes USING btree (thread_id);


--
-- Name: ix_thread_likes_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_thread_likes_user_id ON public.thread_likes USING btree (user_id);


--
-- Name: ix_threads_course_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_threads_course_id ON public.threads USING btree (course_id);


--
-- Name: ix_threads_owner_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_threads_owner_id ON public.threads USING btree (owner_id);


--
-- Name: ix_user_preferences_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX ix_user_preferences_user_id ON public.user_preferences USING btree (user_id);


--
-- Name: ix_users_departments_department_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_users_departments_department_id ON public.users_departments USING btree (department_id);


--
-- Name: ix_users_departments_is_department_admin; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_users_departments_is_department_admin ON public.users_departments USING btree (is_department_admin);


--
-- Name: ix_users_departments_status; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_users_departments_status ON public.users_departments USING btree (status);


--
-- Name: ix_users_departments_user_id; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX ix_users_departments_user_id ON public.users_departments USING btree (user_id);


--
-- Name: ix_users_username; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX ix_users_username ON public.users USING btree (username);


--
-- Name: comment_likes fk_comment_likes_comment_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT fk_comment_likes_comment_id FOREIGN KEY (comment_id) REFERENCES public.thread_comments(id) ON DELETE CASCADE;


--
-- Name: comment_likes fk_comment_likes_user_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.comment_likes
    ADD CONSTRAINT fk_comment_likes_user_id FOREIGN KEY (user_id) REFERENCES public.users(id);


--
-- Name: thread_comments fk_thread_comments_owner_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT fk_thread_comments_owner_id FOREIGN KEY (owner_id) REFERENCES public.users(id);


--
-- Name: thread_comments fk_thread_comments_parent_comment_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT fk_thread_comments_parent_comment_id FOREIGN KEY (parent_comment_id) REFERENCES public.thread_comments(id) ON DELETE CASCADE;


--
-- Name: thread_comments fk_thread_comments_thread_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_comments
    ADD CONSTRAINT fk_thread_comments_thread_id FOREIGN KEY (thread_id) REFERENCES public.threads(id) ON DELETE CASCADE;


--
-- Name: thread_likes fk_thread_likes_thread_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT fk_thread_likes_thread_id FOREIGN KEY (thread_id) REFERENCES public.threads(id) ON DELETE CASCADE;


--
-- Name: thread_likes fk_thread_likes_user_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.thread_likes
    ADD CONSTRAINT fk_thread_likes_user_id FOREIGN KEY (user_id) REFERENCES public.users(id);


--
-- Name: threads fk_threads_course_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT fk_threads_course_id FOREIGN KEY (course_id) REFERENCES public.courses(id);


--
-- Name: threads fk_threads_owner_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.threads
    ADD CONSTRAINT fk_threads_owner_id FOREIGN KEY (owner_id) REFERENCES public.users(id);


--
-- Name: user_preferences fk_user_preferences_user_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.user_preferences
    ADD CONSTRAINT fk_user_preferences_user_id FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


--
-- Name: users_departments fk_users_departments_department_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users_departments
    ADD CONSTRAINT fk_users_departments_department_id FOREIGN KEY (department_id) REFERENCES public.departments(id) ON DELETE CASCADE;


--
-- Name: users_departments fk_users_departments_user_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.users_departments
    ADD CONSTRAINT fk_users_departments_user_id FOREIGN KEY (user_id) REFERENCES public.users(id) ON DELETE CASCADE;


