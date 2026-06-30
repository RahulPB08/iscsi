here to create rust file whic take input from user as follows
1.userid
2.password 12-16 len
3.username

usig this it will run these command

1.`sudo mkdir /var/lib/iscsi_disks` //if not creatd then create ok otherwise not

2.`sudo chmod 755 /var/lib/iscsi_disks`

3.`sudo dd if=/dev/zero of=/var/lib/iscsi_disks/{username.img} bs=1M count={1000}` //here accordig to user get that much of space in to create that size of image image name accoridng to username

4.`sudo chmod 666 /var/lib/iscsi_disks/{username.img}`

5.sudo targetcli
    5.1.go to `cd /`
    5.2.`/backstores/fileio create {username} /var/lib/iscsi_disks/{username.img}`
    5.3`/iscsi create iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:{userid}`
    5.4`cd /iscsi/iqn.2003-01.org.linux-iscsi.rahulbhosle.x8664:{userid}/tpg1`
    5.5`set attribute generate_node_acls=0`
    5.6`acls/ create iqn.1991-05.com.microsoft:{userid}`
    5.7`sudo auth userid={userid}`
    5.8`sudo auth password={password}`
    5.9`luns/ create /backstores/fileio/{username}`
    5.91`cd /`
    5.92`saveconfig`
    5.93`exit`

6.`sudo systemctl restart target` 