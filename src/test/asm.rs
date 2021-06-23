use crate::*;


// generated by build script
include!(concat!(env!("OUT_DIR"), "/test.rs"));


pub struct TestExpectations
{
    has_any: bool,
    output: util::BitVec,
    messages: Vec<TestMessageExpectation>,
}


pub struct TestMessageExpectation
{
    file: String,
    kind: diagn::MessageKind,
    line: usize,
    excerpt: String,
}


pub fn extract_expectations(orig_filename: &str, contents: &str) -> Result<TestExpectations, ()>
{
    let mut expectations = TestExpectations
    {
        has_any: false,
        output: util::BitVec::new(),
        messages: Vec::new(),
    };

    let mut line_num = 0;

    for line in contents.lines()
    {
        if let Some(value_index) = line.find("; =")
        {
            expectations.has_any = true;

            let value_str = line.get((value_index + 3)..).unwrap().trim();
            if value_str != "0x"
            {
                let value = syntax::excerpt_as_bigint(None, value_str, &diagn::Span::new_dummy()).unwrap();
                
                let index = expectations.output.len();
                expectations.output.write_bigint(index, value);
            }
        }
        else if line.find("; error:").is_some() || line.find("; note:").is_some()
        {
            expectations.has_any = true;

            let messages = line
                .get((line.find("; ").unwrap() + 1)..).unwrap()
                .split("/")
                .map(|s| s.trim());

            for message in messages
            {
                let parts = message.split(":").map(|s| s.trim()).collect::<Vec<&str>>();

                let kind = match parts[0]
                {
                    "error" => diagn::MessageKind::Error,
                    "note" => diagn::MessageKind::Note,
                    _ => unreachable!(),
                };

                let (file, line, excerpt) = if parts.len() > 2
                {
                    let file = if parts[1] == "_"
                    {
                        orig_filename.to_string()
                    }
                    else
                    {
                        parts[1].to_string()
                    };

                    (file, parts[2].parse::<usize>().unwrap() - 1, parts[3].to_string())
                }
                else
                {
                    (orig_filename.to_string(), line_num, parts[1].to_string())
                };

                expectations.messages.push(TestMessageExpectation
                {
                    kind,
                    file,
                    line,
                    excerpt,
                });
            }
        }
        else if line.find(";").is_some() && line.find(":").is_some()
        {
            panic!("unrecognized test expectation");
        }

        line_num += 1;
    }

    Ok(expectations)
}


fn populate_fileserver(
    fileserver: &mut util::FileServerMock,
    folder: &std::path::Path,
    cur_folder_name: &str)
{
    for entry in std::fs::read_dir(folder).unwrap()
    {
        let entry = entry.unwrap();
        let path = entry.path();
        let file_stem = path.file_name().unwrap().to_string_lossy();

        if path.is_file()
        {
            let mut filename = cur_folder_name.to_string();
            filename.push_str(&file_stem);

            let contents = std::fs::read_to_string(&path).unwrap();
            fileserver.add(&filename, contents);

            println!("add: {} = {:?}", filename, path);
        }
        else
        {
            let mut new_folder_name = cur_folder_name.to_string();
            new_folder_name.push_str(&file_stem);
            new_folder_name.push_str("/");

            populate_fileserver(fileserver, &path, &new_folder_name);
        }
    }
}


pub fn test_file(filepath: &str)
{
    let path_prefix = std::path::PathBuf::from(&filepath)
        .parent().unwrap()
        .to_path_buf();

	let contents = std::fs::read_to_string(&filepath).unwrap();
	
    let stripped_filename = std::path::PathBuf::from(&filepath)
        .strip_prefix(&path_prefix).unwrap()
        .to_string_lossy()
        .into_owned();

	let expectations = extract_expectations(&stripped_filename, &contents).unwrap();
    if !expectations.has_any
    {
        return;
    }

	let mut fileserver = util::FileServerMock::new();
    populate_fileserver(&mut fileserver, &path_prefix, "");

	let report = diagn::RcReport::new();

	let mut assembler = asm::Assembler::new();
	assembler.register_file(&stripped_filename);
	let maybe_output = assembler.assemble(report.clone(), &mut fileserver, 10);
	
	let mut msgs = Vec::<u8>::new();
	report.print_all(&mut msgs, &fileserver);
    print!("{}", String::from_utf8(msgs).unwrap());
    
    let mut has_msg_mismatch = false;
    for msg in &expectations.messages
    {
        if !report.has_message_at(&fileserver, &msg.file, msg.kind, msg.line, &msg.excerpt)
        {
            println!("\n\
                > test failed -- missing diagnostics message\n\
                > expected: `{}` at file `{}`, line {}\n",
                msg.excerpt, msg.file, msg.line + 1);

            has_msg_mismatch = true;
        }
    }
    
    if has_msg_mismatch
    {
        panic!("test failed");
    }

    // FIXME: Add appropriate submessage expectations for all tests
    if expectations.messages.len() != report.len_with_submessages() &&
        expectations.messages.len() != report.len()
    {
        println!("\n\
            > test failed -- diagnostics mismatch\n\
            > expected {} messages, got {}\n",
            expectations.messages.len(), report.len());
            
        panic!("test failed");
    }
    
    let output = if let Ok(output) = maybe_output
    {
        output.binary
    }
    else
    {
        util::BitVec::new()
    };

    if format!("{:x}", output) != format!("{:x}", expectations.output)
    {
        println!("\n\
            > test failed -- output mismatch\n\
            > got:      0x{:x}\n\
            > expected: 0x{:x}\n",
            &output, &expectations.output);
            
        panic!("test failed");
    }
}