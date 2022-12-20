#define _GNU_SOURCE
#include <stdlib.h>
#include <error.h>
#include <getopt.h>
#include <unistd.h>
#include <sys/types.h>
#include <stdio.h>
#include <string.h>

#include <errno.h>
#include <linux/limits.h>
#include "env.h"
#include "xml_manager.h"
#include "user.h"
#include "capabilities.h"

#ifndef SR_VERSION
#define SR_VERSION "3.0"
#endif

typedef struct _arguments_t {
	char *role;
	int info;
	int version;
	int help;
} arguments_t;

extern char **environ;

/**
 * @brief parse the command line arguments where command is the rest of the command line like this sr (options) command [args]
 * @param argc number of arguments
 * @param argv array of arguments
 * @param args structure to store the parsed arguments
 * @return 0 on success, -1 on error
*/
int parse_arguments(int *argc, char **argv[], arguments_t *args) {
    int c;
    static struct option long_options[] = {
        {"role", required_argument, 0, 'r'},
        {"info", no_argument, 0, 'i'},
        {"version", no_argument, 0, 'v'},
        {"help", no_argument, 0, 'h'},
        {0, 0, 0, 0}
    };

    while ((c = getopt_long(*argc, *argv, "+r:ivh", long_options, NULL)) != -1) {
        switch (c) {
            case 'r':
                args->role = optarg;
                break;
            case 'i':
                args->info = 1;
                break;
            case 'v':
                args->version = 1;
                break;
            case 'h':
                args->help = 1;
                break;
            default:
                return 0;
        }
    }
    *argc -= optind;
    *argv += optind;
    return 1;
}

char *find_absolute_path_from_env(char *file) {
    char *path = strdup(getenv("PATH"));
    if(path == NULL) {
        return NULL;
    }
    char *token = strtok(path, ":");
    char *full_path = NULL;
    while (token != NULL) {
        full_path = malloc(strlen(token) + strlen(file) + 2);
        snprintf(full_path,strlen(token) + strlen(file) + 2, "%s/%s", token, file);
        if (access(full_path, X_OK) == 0) {
            return full_path;
        }
        free(full_path);
        token = strtok(NULL, ":");
    }
    return NULL;
}

void sr_execve(char *command, int p_argc, char *p_argv[], char *p_envp[]) {
    int i = execve(command, p_argv, p_envp);
    if(i == -1 || errno == ENOEXEC) {
        const char **nargv;
    
        nargv = reallocarray(NULL, p_argc + 1, sizeof(char *));
        if (nargv != NULL) {
            nargv[0] = "sh";
            nargv[1] = command;
            memcpy(nargv + 2, p_argv, p_argc * sizeof(char *));
            execve("/bin/sh", (char **)nargv, p_envp);
            free(nargv);
        }
    }
}


/**
 * @brief main function of the SR module
*/
int main(int argc, char *argv[]) {
    extern char **environ;
    arguments_t arguments = {NULL, 0, 0, 0};
    if(!parse_arguments(&argc, &argv, &arguments) || arguments.help) {
        printf("Usage: %s [options] [command [args]]\n",argv[0]);
        printf("Options:\n");
        printf("  -r, --role <role>      Role to use\n");
        printf("  -i, --info             Display rights of executor\n");
        printf("  -v, --version          Display version\n");
        printf("  -h, --help             Display this help\n");
        return 0;
    } else if (arguments.version) {
        printf("SR version %s\n",SR_VERSION);
        return 0;
    }
    
    uid_t euid = geteuid();
    char *user = get_username(euid);
    if(user == NULL) {
        error(1, 0, "Unable to retrieve the username of the executor");
    }
    if(!pam_authenticate_user(user)){
        error(1, 0,"Authentication failed");
    }
    gid_t egid = get_group_id(euid);
    char **groups = NULL;
    int nb_groups = 0;
    if(get_group_names(user, egid, &nb_groups, &groups)) {
        error(1, 0, "Unable to retrieve the groups of the executor");
    }
    
    if(arguments.info) {
        if (arguments.role == NULL)
            print_rights(user,nb_groups,groups,RESTRICTED);
        else{
            print_rights_role(arguments.role,user,nb_groups,groups,RESTRICTED);
        }
        
    }else if(strnlen(argv[0],PATH_MAX)<PATH_MAX){
        char *command = strndup(argv[0],PATH_MAX);
        cap_iab_t iab = NULL;
        options_t options = NULL;
        int ret = get_settings_from_config(user, nb_groups, groups, command, &iab, &options);
        if(!ret) {
            error(1, 0, "Permission denied");
        }
        if(setpcap_effective(1)) {
            error(1, 0, "Unable to setpcap capability");
        }
        if(cap_iab_set_proc(iab)) {
            error(1, 0, "Unable to set capabilities");
        }
        if(setpcap_effective(0)) {
            error(1, 0, "Unable to setpcap capability");
        }
        if (options->no_root) {
			if (activates_securebits()) {
				error(1, 0,"Unable to activate securebits");
			}
		}
        char **env = NULL;
        int res = filter_env_vars(environ, options->env_keep, options->env_check, &env);
        if(res > 0) {
            error(1, 0, "Unable to filter environment variables");
        }
        res = secure_path(getenv("PATH"),options->path);
        if(!res) {
            error(1, 0, "Unable to secure path");
        }
        
        command = realpath(argv[0],NULL);
        if(errno == ENAMETOOLONG){
            error(1, 0, "Path too long");
        }
        if(access(command,X_OK) != 0) {
            command = find_absolute_path_from_env(argv[0]);
            if(command == NULL) {
                error(1, 0, "%s : Command not found", argv[0]);
            }
        }else {
            error(1, 0, "%s : Command not found", argv[0]);
        }
        sr_execve(command, argc, argv, env);
        
    }else{
        error(1, 0, "Command too long");
    }
    free(user);
    free(groups);
    return 0;
}
